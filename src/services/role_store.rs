//! Persisted pubkey → role store for multi-user RBAC (ADR-142).
//!
//! Backs [`crate::models::rbac::UserRole`] with a dedicated `user_roles` SQLite
//! table. The table lives in the same database file as settings (it shares the
//! already-open [`tokio_rusqlite::Connection`]) but is a **separate, isolated
//! table** — deliberately *not* the user-writable `settings` owner layer — so a
//! user cannot escalate their own role through any settings-write endpoint.
//!
//! Identity is the NIP-98-verified pubkey (the user's DID), canonicalised to
//! lowercase hex by [`canonicalise_pubkey`] on **every** path (get/set/remove/
//! bootstrap/lookup) so a mixed-case input always resolves the same row. A
//! process-global handle ([`global_role_store`]) lets `verify_access` resolve
//! roles without threading a new parameter through every call site; it is
//! installed once at startup from `main.rs`.

use std::sync::Arc;

use log::{error, info, warn};
use once_cell::sync::OnceCell;
use rusqlite::OptionalExtension;
use tokio_rusqlite::Connection;

use crate::models::rbac::UserRole;

/// Environment variable naming the bootstrap Owner pubkey (hex). When set, the
/// pubkey is granted `Owner` on store initialisation if it has no explicit row.
pub const RBAC_OWNER_PUBKEY_ENV: &str = "RBAC_OWNER_PUBKEY";

/// Escape hatch permitting startup with no Owner assigned (legacy single-user
/// deployments where the `POWER_USER_PUBKEYS` → Admin fallback is the intended
/// reality). Without it, [`RoleStore::has_owner`] returning false is a
/// fail-closed startup error (see `main.rs`).
pub const RBAC_ALLOW_OWNERLESS_ENV: &str = "RBAC_ALLOW_OWNERLESS";

/// Role granted to an authenticated pubkey with no explicit assignment.
/// Accepted values: `editor` (compatibility default — preserves pre-RBAC
/// behaviour where every authenticated user could write) and `viewer`
/// (multi-user-locked posture: unknown signers read only until an Admin
/// grants a role). Any other value fails closed to `viewer` with an error
/// log — a typo must never widen access.
pub const RBAC_DEFAULT_ROLE_ENV: &str = "RBAC_DEFAULT_ROLE";

/// Column DDL shared by the initial `CREATE TABLE` and the legacy-migration
/// rebuild. `CHECK`-constrained: `role` is confined to the canonical lattice and
/// `pubkey` to a 64-char lowercase-hex string.
const TABLE_COLUMNS: &str = r#"(
    pubkey      TEXT PRIMARY KEY NOT NULL
                CHECK (length(pubkey) = 64 AND pubkey NOT GLOB '*[^0-9a-f]*'),
    role        TEXT NOT NULL
                CHECK (role IN ('owner','admin','editor','viewer')),
    assigned_by TEXT,
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
)"#;

/// Errors from role storage. An unparseable stored role is a *hard error*
/// (`InvalidRole`) — never silently coerced to a default that could grant
/// elevated access.
#[derive(Debug, thiserror::Error)]
pub enum RoleStoreError {
    #[error("role store database error: {0}")]
    Db(#[from] tokio_rusqlite::Error),
    #[error("invalid stored role '{role}' for pubkey {pubkey}")]
    InvalidRole { pubkey: String, role: String },
    #[error("invalid pubkey (must be 64 hex chars): {0}")]
    InvalidPubkey(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("refused: this would remove the last Owner")]
    LastOwner,
}

/// Canonicalise a pubkey for storage AND lookup: trim + lowercase. This is the
/// **single** function every path uses so mixed-case input hits the same row.
/// It does *not* validate length/charset — that is [`validate_pubkey`], applied
/// only on writes (a lookup of an arbitrary pubkey must not error).
pub fn canonicalise_pubkey(pubkey: &str) -> String {
    pubkey.trim().to_ascii_lowercase()
}

/// Canonicalise and validate a pubkey to canonical 64-char lowercase hex.
fn validate_pubkey(pubkey: &str) -> Result<String, RoleStoreError> {
    let p = canonicalise_pubkey(pubkey);
    if p.len() == 64 && p.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(p)
    } else {
        Err(RoleStoreError::InvalidPubkey(pubkey.to_string()))
    }
}

/// A single persisted role assignment, as returned by [`RoleStore::list`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoleAssignment {
    pub pubkey: String,
    pub role: UserRole,
    pub assigned_by: Option<String>,
    pub updated_at: i64,
}

/// SQLite-backed store mapping pubkeys to [`UserRole`]s.
pub struct RoleStore {
    conn: Arc<Connection>,
    /// Role for authenticated-but-unassigned pubkeys, resolved once at
    /// construction from [`RBAC_DEFAULT_ROLE_ENV`]. See
    /// [`resolve_default_role`] for the parse/fail-closed rules.
    default_role: UserRole,
}

/// Parse an [`RBAC_DEFAULT_ROLE_ENV`] value into the unassigned-signer
/// default role.
///
/// - `None` (unset) / explicitly empty `""` → `Editor` (ADR-2010 compatibility
///   default; compose `${VAR:-editor}` interpolation makes empty ≡ unset)
/// - `editor` → `Editor`; `viewer` → `Viewer` (case-insensitive, trimmed)
/// - anything else present — including whitespace-only — → **fail closed** to
///   `Viewer` with an error log. `admin` and `owner` are deliberately rejected
///   here too: an env var must never be able to mass-grant elevated access.
fn parse_default_role(value: Option<&str>) -> UserRole {
    match value {
        None => UserRole::default_authenticated(),
        Some("") => UserRole::default_authenticated(),
        Some(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "editor" => UserRole::Editor,
            "viewer" => UserRole::Viewer,
            other => {
                error!(
                    "RBAC: {RBAC_DEFAULT_ROLE_ENV}='{other}' is not one of \
                     'editor'|'viewer'; failing closed to Viewer"
                );
                UserRole::Viewer
            }
        },
    }
}

/// Read and parse [`RBAC_DEFAULT_ROLE_ENV`] from the process environment.
/// A present-but-non-UTF-8 value is a *present invalid* value and fails closed
/// to `Viewer` — it must not be conflated with "unset" (which would widen to
/// Editor).
fn resolve_default_role() -> UserRole {
    match std::env::var(RBAC_DEFAULT_ROLE_ENV) {
        Ok(v) => parse_default_role(Some(&v)),
        Err(std::env::VarError::NotPresent) => parse_default_role(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            error!(
                "RBAC: {RBAC_DEFAULT_ROLE_ENV} is set but not valid UTF-8; \
                 failing closed to Viewer"
            );
            UserRole::Viewer
        }
    }
}

impl RoleStore {
    /// Construct over an already-open connection, ensuring the `user_roles`
    /// table exists **and carries the `CHECK` constraints** — migrating a legacy
    /// (pre-constraint) table by rebuild if necessary.
    pub async fn new(conn: Arc<Connection>) -> Result<Self, tokio_rusqlite::Error> {
        let (migrated, kept, total) = conn
            .call(|c| {
                let existing_sql: Option<String> = c
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE type='table' AND name='user_roles'",
                        [],
                        |r| r.get(0),
                    )
                    .optional()?;

                match existing_sql {
                    // Fresh database — create the constrained table.
                    None => {
                        c.execute_batch(&format!("CREATE TABLE user_roles {TABLE_COLUMNS};"))?;
                        Ok((false, 0i64, 0i64))
                    }
                    // Already constrained — nothing to do.
                    Some(sql) if sql.contains("CHECK") => Ok((false, 0i64, 0i64)),
                    // Legacy unconstrained table — rebuild inside a transaction,
                    // validating existing rows (invalid ones are dropped and the
                    // user reverts to the default role — fail closed).
                    Some(_) => {
                        let tx = c.transaction()?;
                        let total: i64 =
                            tx.query_row("SELECT COUNT(*) FROM user_roles", [], |r| r.get(0))?;
                        tx.execute_batch(&format!("CREATE TABLE user_roles_new {TABLE_COLUMNS};"))?;
                        let kept = tx.execute(
                            "INSERT INTO user_roles_new (pubkey, role, assigned_by, updated_at)
                             SELECT lower(pubkey), lower(role), assigned_by, updated_at
                             FROM user_roles
                             WHERE lower(role) IN ('owner','admin','editor','viewer')
                               AND length(pubkey) = 64
                               AND lower(pubkey) NOT GLOB '*[^0-9a-f]*'",
                            [],
                        )? as i64;
                        tx.execute_batch(
                            "DROP TABLE user_roles;
                             ALTER TABLE user_roles_new RENAME TO user_roles;",
                        )?;
                        tx.commit()?;
                        Ok((true, kept, total))
                    }
                }
            })
            .await?;

        if migrated {
            if kept < total {
                warn!(
                    "RBAC: migrated legacy user_roles table to constrained schema; \
                     kept {kept}/{total} rows ({} invalid dropped, reverting to default)",
                    total - kept
                );
            } else {
                info!("RBAC: migrated legacy user_roles table to constrained schema ({kept} rows)");
            }
        }
        let default_role = resolve_default_role();
        if default_role != UserRole::default_authenticated() {
            info!(
                "RBAC: unassigned-signer default role is {} (via {RBAC_DEFAULT_ROLE_ENV})",
                default_role.as_str()
            );
        }
        Ok(Self { conn, default_role })
    }

    /// The role an authenticated-but-unassigned pubkey resolves to, as
    /// configured by [`RBAC_DEFAULT_ROLE_ENV`] at construction.
    pub fn configured_default(&self) -> UserRole {
        self.default_role
    }

    /// Resolve the explicit role for `pubkey`, or `None` if unassigned. A row
    /// whose stored role does not parse is a hard [`RoleStoreError::InvalidRole`].
    pub async fn get(&self, pubkey: &str) -> Result<Option<UserRole>, RoleStoreError> {
        let key = canonicalise_pubkey(pubkey);
        let key_for_err = key.clone();
        let raw: Option<String> = self
            .conn
            .call(move |c| {
                let mut stmt = c.prepare_cached("SELECT role FROM user_roles WHERE pubkey = ?1")?;
                let mut rows = stmt.query([key])?;
                match rows.next()? {
                    Some(row) => Ok(Some(row.get::<_, String>(0)?)),
                    None => Ok(None),
                }
            })
            .await?;
        match raw {
            None => Ok(None),
            Some(s) => UserRole::parse(&s)
                .map(Some)
                .ok_or(RoleStoreError::InvalidRole {
                    pubkey: key_for_err,
                    role: s,
                }),
        }
    }

    /// Resolve the *effective* role for an authenticated user.
    ///
    /// Precedence: explicit assignment → `Admin` for legacy power users → else
    /// the configured unassigned-signer default ([`RBAC_DEFAULT_ROLE_ENV`],
    /// `Editor` unless overridden). **Fails closed** to `Viewer` on any error
    /// (including an invalid stored role) — never up to Admin/power-user.
    pub async fn effective_role(&self, pubkey: &str, is_power_user: bool) -> UserRole {
        match self.get(pubkey).await {
            Ok(Some(role)) => role,
            Ok(None) => {
                if is_power_user {
                    UserRole::Admin
                } else {
                    self.default_role
                }
            }
            Err(e) => {
                error!("RBAC: role lookup failed for {pubkey}: {e}; failing closed to Viewer");
                UserRole::Viewer
            }
        }
    }

    /// Upsert `pubkey`'s role. `pubkey` is validated to canonical 64-hex.
    pub async fn set(
        &self,
        pubkey: &str,
        role: UserRole,
        assigned_by: Option<&str>,
    ) -> Result<UserRole, RoleStoreError> {
        let key = validate_pubkey(pubkey)?;
        let role_str = role.as_str().to_string();
        let assigned_by = assigned_by.map(|s| canonicalise_pubkey(s));
        self.conn
            .call(move |c| {
                c.execute(
                    "INSERT INTO user_roles (pubkey, role, assigned_by, updated_at)
                     VALUES (?1, ?2, ?3, strftime('%s','now'))
                     ON CONFLICT(pubkey) DO UPDATE SET
                        role = excluded.role,
                        assigned_by = excluded.assigned_by,
                        updated_at = excluded.updated_at",
                    rusqlite::params![key, role_str, assigned_by],
                )?;
                Ok(())
            })
            .await?;
        Ok(role)
    }

    /// Assign `new_role` to `target`, enforcing the full authorization lattice
    /// **and** the last-Owner invariant **atomically** in one transaction
    /// (Codex round-2): the current-role read, the `can_assign` checks, the
    /// last-Owner count, and the write cannot interleave with a concurrent
    /// mutation.
    pub async fn assign_checked(
        &self,
        target: &str,
        new_role: UserRole,
        caller: UserRole,
        caller_pubkey: &str,
    ) -> Result<UserRole, RoleStoreError> {
        let key = validate_pubkey(target)?;
        let assigned_by = canonicalise_pubkey(caller_pubkey);
        let new_str = new_role.as_str().to_string();

        let outcome: TxOutcome = self
            .conn
            .call(move |c| {
                let tx = c.transaction()?;
                let existing = read_role(&tx, &key)?;
                if let ExistingRole::Invalid(role) = existing {
                    return Ok(TxOutcome::InvalidExisting { pubkey: key, role });
                }
                let existing = existing.into_option();

                // Authorization: caller must be able to assign the NEW role, and
                // (if the target already holds one) their CURRENT role too.
                if !caller.can_assign(new_role) {
                    return Ok(TxOutcome::Forbidden(format!(
                        "{caller} may not assign role {new_role}"
                    )));
                }
                if let Some(ex) = existing {
                    if !caller.can_assign(ex) {
                        return Ok(TxOutcome::Forbidden(format!(
                            "{caller} may not modify a user currently holding {ex}"
                        )));
                    }
                }

                // Last-Owner invariant: demoting the sole Owner is refused.
                if existing == Some(UserRole::Owner) && new_role != UserRole::Owner {
                    let owners: i64 = tx.query_row(
                        "SELECT COUNT(*) FROM user_roles WHERE role = 'owner'",
                        [],
                        |r| r.get(0),
                    )?;
                    if owners <= 1 {
                        return Ok(TxOutcome::LastOwner);
                    }
                }

                tx.execute(
                    "INSERT INTO user_roles (pubkey, role, assigned_by, updated_at)
                     VALUES (?1, ?2, ?3, strftime('%s','now'))
                     ON CONFLICT(pubkey) DO UPDATE SET
                        role = excluded.role,
                        assigned_by = excluded.assigned_by,
                        updated_at = excluded.updated_at",
                    rusqlite::params![key, new_str, assigned_by],
                )?;
                tx.commit()?;
                Ok(TxOutcome::Ok)
            })
            .await?;

        outcome.into_result(new_role)
    }

    /// Remove an explicit assignment (revert to default), enforcing `can_assign`
    /// on the current role and the last-Owner invariant, atomically.
    pub async fn revoke_checked(
        &self,
        target: &str,
        caller: UserRole,
    ) -> Result<bool, RoleStoreError> {
        let key = canonicalise_pubkey(target);

        let outcome: TxOutcome = self
            .conn
            .call(move |c| {
                let tx = c.transaction()?;
                let existing = read_role(&tx, &key)?;
                if let ExistingRole::Invalid(role) = existing {
                    return Ok(TxOutcome::InvalidExisting { pubkey: key, role });
                }
                let existing = existing.into_option();
                let Some(ex) = existing else {
                    // Nothing to remove — succeed as a no-op (had_explicit=false).
                    return Ok(TxOutcome::NoOp);
                };
                if !caller.can_assign(ex) {
                    return Ok(TxOutcome::Forbidden(format!(
                        "{caller} may not revoke a user currently holding {ex}"
                    )));
                }
                if ex == UserRole::Owner {
                    let owners: i64 = tx.query_row(
                        "SELECT COUNT(*) FROM user_roles WHERE role = 'owner'",
                        [],
                        |r| r.get(0),
                    )?;
                    if owners <= 1 {
                        return Ok(TxOutcome::LastOwner);
                    }
                }
                tx.execute("DELETE FROM user_roles WHERE pubkey = ?1", [key])?;
                tx.commit()?;
                Ok(TxOutcome::Ok)
            })
            .await?;

        match outcome {
            TxOutcome::Ok => Ok(true),
            TxOutcome::NoOp => Ok(false),
            TxOutcome::Forbidden(m) => Err(RoleStoreError::Forbidden(m)),
            TxOutcome::LastOwner => Err(RoleStoreError::LastOwner),
            TxOutcome::InvalidExisting { pubkey, role } => {
                Err(RoleStoreError::InvalidRole { pubkey, role })
            }
        }
    }

    /// Whether any pubkey currently holds `Owner`. Used by the startup lockout
    /// guard.
    pub async fn has_owner(&self) -> Result<bool, RoleStoreError> {
        let exists: i64 = self
            .conn
            .call(|c| {
                let mut stmt = c.prepare_cached(
                    "SELECT EXISTS(SELECT 1 FROM user_roles WHERE role = 'owner')",
                )?;
                let mut rows = stmt.query([])?;
                match rows.next()? {
                    Some(row) => Ok(row.get::<_, i64>(0)?),
                    None => Ok(0),
                }
            })
            .await?;
        Ok(exists != 0)
    }

    /// List every explicit assignment, most-recently-updated first.
    pub async fn list(&self) -> Result<Vec<RoleAssignment>, RoleStoreError> {
        let rows = self
            .conn
            .call(|c| {
                let mut stmt = c.prepare_cached(
                    "SELECT pubkey, role, assigned_by, updated_at
                     FROM user_roles ORDER BY updated_at DESC",
                )?;
                let mapped = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?;
                let mut out = Vec::new();
                for r in mapped {
                    out.push(r?);
                }
                Ok(out)
            })
            .await?;

        let mut assignments = Vec::with_capacity(rows.len());
        for (pubkey, role, assigned_by, updated_at) in rows {
            let role = UserRole::parse(&role).ok_or_else(|| RoleStoreError::InvalidRole {
                pubkey: pubkey.clone(),
                role,
            })?;
            assignments.push(RoleAssignment {
                pubkey,
                role,
                assigned_by,
                updated_at,
            });
        }
        Ok(assignments)
    }

    /// Grant `Owner` to the pubkey named by [`RBAC_OWNER_PUBKEY_ENV`] if it has
    /// no explicit role yet. Idempotent; a no-op when the env var is unset.
    pub async fn bootstrap_owner_from_env(&self) -> Result<(), RoleStoreError> {
        let Ok(owner) = std::env::var(RBAC_OWNER_PUBKEY_ENV) else {
            return Ok(());
        };
        if owner.trim().is_empty() {
            return Ok(());
        }
        match validate_pubkey(&owner) {
            Ok(key) => {
                if self.get(&key).await?.is_none() {
                    self.set(&key, UserRole::Owner, Some("bootstrap:env"))
                        .await?;
                    info!("RBAC: bootstrapped Owner role for {key} from {RBAC_OWNER_PUBKEY_ENV}");
                }
                Ok(())
            }
            Err(_) => {
                warn!(
                    "RBAC: {RBAC_OWNER_PUBKEY_ENV} is not a valid 64-hex pubkey ('{owner}'); skipping Owner bootstrap"
                );
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction helpers
// ---------------------------------------------------------------------------

/// The current role of a target row as read inside a transaction.
enum ExistingRole {
    None,
    Some(UserRole),
    Invalid(String),
}

impl ExistingRole {
    fn into_option(self) -> Option<UserRole> {
        match self {
            ExistingRole::Some(r) => Some(r),
            _ => None,
        }
    }
}

/// Read a target's role within a transaction, classifying an unparseable stored
/// value as `Invalid` (a hard error surfaced to the caller).
fn read_role(tx: &rusqlite::Transaction<'_>, pubkey: &str) -> rusqlite::Result<ExistingRole> {
    let raw: Option<String> = tx
        .query_row(
            "SELECT role FROM user_roles WHERE pubkey = ?1",
            [pubkey],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match raw {
        None => ExistingRole::None,
        Some(s) => match UserRole::parse(&s) {
            Some(r) => ExistingRole::Some(r),
            None => ExistingRole::Invalid(s),
        },
    })
}

/// Outcome of a transactional check+mutate, mapped to [`RoleStoreError`] outside
/// the `conn.call` closure (whose error type cannot carry our variants).
enum TxOutcome {
    Ok,
    NoOp,
    Forbidden(String),
    LastOwner,
    InvalidExisting { pubkey: String, role: String },
}

impl TxOutcome {
    fn into_result(self, ok_role: UserRole) -> Result<UserRole, RoleStoreError> {
        match self {
            TxOutcome::Ok | TxOutcome::NoOp => Ok(ok_role),
            TxOutcome::Forbidden(m) => Err(RoleStoreError::Forbidden(m)),
            TxOutcome::LastOwner => Err(RoleStoreError::LastOwner),
            TxOutcome::InvalidExisting { pubkey, role } => {
                Err(RoleStoreError::InvalidRole { pubkey, role })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Process-global handle
// ---------------------------------------------------------------------------

static GLOBAL_ROLE_STORE: OnceCell<Arc<RoleStore>> = OnceCell::new();

/// Install the process-global [`RoleStore`]. Called once at startup.
pub fn set_global_role_store(store: Arc<RoleStore>) -> Result<(), Arc<RoleStore>> {
    GLOBAL_ROLE_STORE.set(store)
}

/// Fetch the process-global [`RoleStore`], if installed.
pub fn global_role_store() -> Option<Arc<RoleStore>> {
    GLOBAL_ROLE_STORE.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PK_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PK_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const PK_PU: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    // Same key as PK_A but upper-case, to prove canonicalisation.
    const PK_A_UPPER: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    async fn mem_store() -> RoleStore {
        let conn = Connection::open_in_memory()
            .await
            .expect("open in-memory db");
        RoleStore::new(Arc::new(conn)).await.expect("create table")
    }

    #[tokio::test]
    async fn unassigned_user_gets_editor_default() {
        let store = mem_store().await;
        assert_eq!(store.get(PK_A).await.unwrap(), None);
        assert_eq!(store.effective_role(PK_A, false).await, UserRole::Editor);
    }

    #[test]
    fn default_role_parse_lattice() {
        // Unset and explicitly-empty preserve the ADR-2010 compatibility
        // default (compose `${VAR:-editor}` makes empty ≡ unset).
        assert_eq!(parse_default_role(None), UserRole::Editor);
        assert_eq!(parse_default_role(Some("")), UserRole::Editor);
        // Explicit values, case-insensitive.
        assert_eq!(parse_default_role(Some("editor")), UserRole::Editor);
        assert_eq!(parse_default_role(Some("Viewer")), UserRole::Viewer);
        assert_eq!(parse_default_role(Some(" viewer ")), UserRole::Viewer);
        // Present-but-garbage fails CLOSED to Viewer — an env var must never
        // widen access. Whitespace-only is present garbage, not "unset".
        assert_eq!(parse_default_role(Some("  ")), UserRole::Viewer);
        assert_eq!(parse_default_role(Some("admin")), UserRole::Viewer);
        assert_eq!(parse_default_role(Some("owner")), UserRole::Viewer);
        assert_eq!(parse_default_role(Some("banana")), UserRole::Viewer);
    }

    #[tokio::test]
    async fn unassigned_default_is_store_configured_not_hardcoded() {
        // The store's Ok(None) branch must consult the constructed default,
        // not UserRole::default_authenticated() directly.
        let conn = Connection::open_in_memory().await.expect("open db");
        let mut store = RoleStore::new(Arc::new(conn)).await.expect("create");
        store.default_role = UserRole::Viewer;
        assert_eq!(store.effective_role(PK_A, false).await, UserRole::Viewer);
        assert_eq!(store.configured_default(), UserRole::Viewer);
        // Power-user precedence is unaffected by the default.
        assert_eq!(store.effective_role(PK_PU, true).await, UserRole::Admin);
        // An explicit assignment still wins over the default.
        store.set(PK_A, UserRole::Editor, None).await.unwrap();
        assert_eq!(store.effective_role(PK_A, false).await, UserRole::Editor);
    }

    #[tokio::test]
    async fn power_user_without_row_maps_to_admin() {
        let store = mem_store().await;
        assert_eq!(store.effective_role(PK_PU, true).await, UserRole::Admin);
    }

    #[tokio::test]
    async fn explicit_role_overrides_power_user_flag() {
        let store = mem_store().await;
        store
            .set(PK_PU, UserRole::Viewer, Some(PK_A))
            .await
            .unwrap();
        assert_eq!(store.effective_role(PK_PU, true).await, UserRole::Viewer);
    }

    #[tokio::test]
    async fn set_get_roundtrip_and_upsert() {
        let store = mem_store().await;
        store.set(PK_A, UserRole::Admin, Some(PK_B)).await.unwrap();
        assert_eq!(store.get(PK_A).await.unwrap(), Some(UserRole::Admin));
        store.set(PK_A, UserRole::Editor, Some(PK_B)).await.unwrap();
        assert_eq!(store.get(PK_A).await.unwrap(), Some(UserRole::Editor));
    }

    #[tokio::test]
    async fn mixed_case_pubkey_hits_same_row() {
        let store = mem_store().await;
        // Store under upper-case, read under lower-case (and vice versa).
        store.set(PK_A_UPPER, UserRole::Viewer, None).await.unwrap();
        assert_eq!(store.get(PK_A).await.unwrap(), Some(UserRole::Viewer));
        assert_eq!(
            store.effective_role(PK_A, true).await,
            UserRole::Viewer,
            "canonicalised lookup must find the explicit Viewer row"
        );
        // Only one row exists — the two cases did not create duplicates.
        assert_eq!(store.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn remove_reverts_to_default() {
        let store = mem_store().await;
        store.set(PK_A, UserRole::Editor, None).await.unwrap();
        assert!(store.revoke_checked(PK_A, UserRole::Owner).await.unwrap());
        assert_eq!(store.get(PK_A).await.unwrap(), None);
        assert!(!store.revoke_checked(PK_A, UserRole::Owner).await.unwrap());
    }

    #[tokio::test]
    async fn list_returns_all_assignments() {
        let store = mem_store().await;
        store.set(PK_A, UserRole::Owner, None).await.unwrap();
        store.set(PK_B, UserRole::Viewer, Some(PK_A)).await.unwrap();
        let all = store.list().await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all
            .iter()
            .any(|r| r.pubkey == PK_A && r.role == UserRole::Owner));
        assert!(all
            .iter()
            .any(|r| r.pubkey == PK_B && r.role == UserRole::Viewer));
    }

    #[tokio::test]
    async fn has_owner_tracks_owner_presence() {
        let store = mem_store().await;
        assert!(!store.has_owner().await.unwrap());
        store.set(PK_A, UserRole::Admin, None).await.unwrap();
        assert!(!store.has_owner().await.unwrap(), "Admin is not Owner");
        store.set(PK_B, UserRole::Owner, None).await.unwrap();
        assert!(store.has_owner().await.unwrap());
    }

    #[tokio::test]
    async fn set_rejects_non_hex_pubkey() {
        let store = mem_store().await;
        let err = store.set("not-a-hex-pubkey", UserRole::Editor, None).await;
        assert!(matches!(err, Err(RoleStoreError::InvalidPubkey(_))));
    }

    #[tokio::test]
    async fn demoting_last_owner_is_rejected() {
        let store = mem_store().await;
        store.set(PK_A, UserRole::Owner, None).await.unwrap();
        // Owner demotes themselves → refused (they are the last Owner).
        let err = store
            .assign_checked(PK_A, UserRole::Admin, UserRole::Owner, PK_A)
            .await;
        assert!(matches!(err, Err(RoleStoreError::LastOwner)));
        // Revoking the last Owner is likewise refused.
        let rev = store.revoke_checked(PK_A, UserRole::Owner).await;
        assert!(matches!(rev, Err(RoleStoreError::LastOwner)));
        // With a second Owner, demotion of the first is allowed.
        store.set(PK_B, UserRole::Owner, None).await.unwrap();
        assert_eq!(
            store
                .assign_checked(PK_A, UserRole::Admin, UserRole::Owner, PK_B)
                .await
                .unwrap(),
            UserRole::Admin
        );
    }

    #[tokio::test]
    async fn assign_checked_enforces_can_assign() {
        let store = mem_store().await;
        // An Admin cannot mint an Admin/Owner.
        assert!(matches!(
            store
                .assign_checked(PK_A, UserRole::Admin, UserRole::Admin, PK_B)
                .await,
            Err(RoleStoreError::Forbidden(_))
        ));
        // ...but can grant Editor.
        assert_eq!(
            store
                .assign_checked(PK_A, UserRole::Editor, UserRole::Admin, PK_B)
                .await
                .unwrap(),
            UserRole::Editor
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_unknown_role() {
        let store = mem_store().await;
        let bad = store
            .conn
            .call(|c| {
                c.execute(
                    "INSERT INTO user_roles (pubkey, role) VALUES (?1, 'superuser')",
                    [PK_A],
                )?;
                Ok(())
            })
            .await;
        assert!(bad.is_err(), "CHECK constraint must reject an unknown role");
    }

    #[tokio::test]
    async fn invalid_stored_role_is_hard_error_and_fails_closed() {
        // Legacy/corrupt table WITHOUT the CHECK, with a bad role. new() must
        // migrate it (dropping the invalid row), so the corrupt role cannot
        // survive; get() then returns None and effective_role is the default.
        let conn = Connection::open_in_memory().await.unwrap();
        conn.call(|c| {
            c.execute_batch(
                "CREATE TABLE user_roles (pubkey TEXT PRIMARY KEY, role TEXT, \
                 assigned_by TEXT, updated_at INTEGER DEFAULT 0);",
            )?;
            Ok(())
        })
        .await
        .unwrap();
        let key = PK_A.to_string();
        conn.call(move |c| {
            c.execute(
                "INSERT INTO user_roles (pubkey, role) VALUES (?1, 'superuser')",
                [key],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        let store = RoleStore::new(Arc::new(conn)).await.unwrap();
        // Migration dropped the invalid row (fail closed) — no corrupt role
        // survives, and the user reverts to the default.
        assert_eq!(store.get(PK_A).await.unwrap(), None);
        assert_eq!(store.effective_role(PK_A, true).await, UserRole::Admin);
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn legacy_table_migration_applies_constraints() {
        // A legacy table with a MIX of valid and invalid rows: valid ones are
        // preserved (canonicalised), invalid ones dropped, and the rebuilt table
        // enforces the CHECK afterwards.
        let conn = Connection::open_in_memory().await.unwrap();
        conn.call(|c| {
            c.execute_batch(
                "CREATE TABLE user_roles (pubkey TEXT PRIMARY KEY, role TEXT, \
                 assigned_by TEXT, updated_at INTEGER DEFAULT 0);
                 INSERT INTO user_roles (pubkey, role) VALUES
                    ('AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','owner'),
                    ('short','admin'),
                    ('dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd','bogus');",
            )?;
            Ok(())
        })
        .await
        .unwrap();

        let store = RoleStore::new(Arc::new(conn)).await.unwrap();
        // Only the valid Owner survived, canonicalised to lowercase.
        let all = store.list().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].role, UserRole::Owner);
        assert_eq!(all[0].pubkey, PK_A);
        assert!(store.has_owner().await.unwrap());
        // The rebuilt table enforces the CHECK.
        let bad = store
            .conn
            .call(|c| {
                c.execute(
                    "INSERT INTO user_roles (pubkey, role) VALUES (?1, 'superuser')",
                    [PK_B],
                )?;
                Ok(())
            })
            .await;
        assert!(bad.is_err(), "migrated table must enforce the role CHECK");
    }
}
