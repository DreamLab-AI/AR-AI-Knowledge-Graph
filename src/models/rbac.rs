//! Multi-user RBAC role model (ADR-142)
//!
//! Defines the persisted, DID-bound role lattice for VisionClaw's multi-user
//! authorization. Unlike the reference `enterprise_auth.rs` on the
//! `sprint-3/jss-cut-scaffold` branch — which read a spoofable
//! `X-Enterprise-Role` header — roles here are bound to the cryptographically
//! verified NIP-98 pubkey (the user's decentralized identifier) and persisted
//! in SQLite via [`crate::services::role_store::RoleStore`].
//!
//! ## The lattice
//!
//! ```text
//!   Owner (4)  ── full control, can grant/revoke Admin and Owner
//!   Admin (3)  ── manage users/settings, cannot touch Owner grants
//!   Editor (2) ── read + mutate graph/content
//!   Viewer (1) ── read-only
//! ```
//!
//! Higher numeric level satisfies every lower requirement. The model maps onto
//! the pre-existing [`crate::utils::auth::AccessLevel`] machinery so that all
//! current route guards (`verify_access`, `RequireAuth`) keep working while
//! gaining persisted per-user resolution.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::utils::auth::AccessLevel;

/// A persisted, pubkey-bound authorization role.
///
/// Serialized as a lowercase string (`"owner"`, `"admin"`, `"editor"`,
/// `"viewer"`) for storage and API payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Read-only access.
    Viewer,
    /// Read + graph/content mutation (the default for an authenticated user,
    /// preserving the historical "any authenticated user may write graph"
    /// behaviour of `AccessLevel::Authenticated`).
    Editor,
    /// User & settings management; everything below Owner.
    Admin,
    /// Full control, including granting/revoking Admin and Owner.
    Owner,
}

impl UserRole {
    /// Numeric privilege level. Higher is more privileged.
    pub fn level(&self) -> u8 {
        match self {
            UserRole::Viewer => 1,
            UserRole::Editor => 2,
            UserRole::Admin => 3,
            UserRole::Owner => 4,
        }
    }

    /// The default role assigned to an authenticated user who has never been
    /// explicitly granted a role.
    ///
    /// Chosen as `Editor` (not `Viewer`) deliberately: `main`'s pre-RBAC
    /// `verify_access` mapped every authenticated NIP-98 user to
    /// `AccessLevel::Authenticated` (read + write-graph). Defaulting to `Editor`
    /// preserves that behaviour exactly, so enabling RBAC does not silently
    /// revoke write access from existing users. `Viewer` is an explicit,
    /// admin-applied downgrade.
    pub const fn default_authenticated() -> Self {
        UserRole::Editor
    }

    /// Does holding this role satisfy a `required` minimum role?
    pub fn satisfies(&self, required: UserRole) -> bool {
        self.level() >= required.level()
    }

    /// Map the role onto the legacy [`AccessLevel`] lattice used by the existing
    /// route guards. This is what lets the four-tier model drive
    /// `verify_access` without rewriting every call site.
    ///
    /// - `Owner`/`Admin` → `Admin` (full permissions incl. settings writes)
    /// - `Editor`        → `Authenticated` (read + graph writes, no settings)
    /// - `Viewer`        → `ReadOnly`
    pub fn to_access_level(&self) -> AccessLevel {
        match self {
            UserRole::Owner | UserRole::Admin => AccessLevel::Admin,
            UserRole::Editor => AccessLevel::Authenticated,
            UserRole::Viewer => AccessLevel::ReadOnly,
        }
    }

    /// Parse from the canonical lowercase wire string. Accepts a few friendly
    /// aliases. Returns `None` for anything unrecognised.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "owner" => Some(UserRole::Owner),
            "admin" | "administrator" => Some(UserRole::Admin),
            "editor" | "contributor" | "write" => Some(UserRole::Editor),
            "viewer" | "read" | "readonly" | "read-only" => Some(UserRole::Viewer),
            _ => None,
        }
    }

    /// Canonical lowercase wire string.
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Owner => "owner",
            UserRole::Admin => "admin",
            UserRole::Editor => "editor",
            UserRole::Viewer => "viewer",
        }
    }

    /// Whether a caller holding `self` may *assign* the `target` role to another
    /// user. Rules (least-privilege, no self-escalation via delegation):
    ///
    /// - Only an `Owner` may grant or revoke `Owner` or `Admin`.
    /// - An `Admin` may grant `Editor` or `Viewer` (i.e. strictly below Admin).
    /// - `Editor`/`Viewer` may assign nothing.
    pub fn can_assign(&self, target: UserRole) -> bool {
        match self {
            UserRole::Owner => true,
            UserRole::Admin => target.level() < UserRole::Admin.level(),
            _ => false,
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_ordering_is_monotonic() {
        assert!(UserRole::Owner.level() > UserRole::Admin.level());
        assert!(UserRole::Admin.level() > UserRole::Editor.level());
        assert!(UserRole::Editor.level() > UserRole::Viewer.level());
        // Derived Ord must agree with level().
        assert!(UserRole::Owner > UserRole::Admin);
        assert!(UserRole::Admin > UserRole::Editor);
        assert!(UserRole::Editor > UserRole::Viewer);
    }

    #[test]
    fn satisfies_is_reflexive_and_hierarchical() {
        for r in [
            UserRole::Viewer,
            UserRole::Editor,
            UserRole::Admin,
            UserRole::Owner,
        ] {
            assert!(r.satisfies(r), "{r} must satisfy itself");
            assert!(r.satisfies(UserRole::Viewer), "{r} must satisfy Viewer");
        }
        assert!(UserRole::Owner.satisfies(UserRole::Admin));
        assert!(!UserRole::Editor.satisfies(UserRole::Admin));
        assert!(!UserRole::Viewer.satisfies(UserRole::Editor));
    }

    #[test]
    fn access_level_mapping_preserves_privilege() {
        // Owner/Admin reach settings + admin.
        assert!(UserRole::Owner
            .to_access_level()
            .has_permission(&AccessLevel::WriteSettings));
        assert!(UserRole::Admin
            .to_access_level()
            .has_permission(&AccessLevel::Admin));
        // Editor writes graph but not settings/admin (the escalation boundary).
        let editor = UserRole::Editor.to_access_level();
        assert!(editor.has_permission(&AccessLevel::WriteGraph));
        assert!(!editor.has_permission(&AccessLevel::WriteSettings));
        assert!(!editor.has_permission(&AccessLevel::Admin));
        // Viewer reads only.
        let viewer = UserRole::Viewer.to_access_level();
        assert!(viewer.has_permission(&AccessLevel::ReadOnly));
        assert!(!viewer.has_permission(&AccessLevel::WriteGraph));
    }

    #[test]
    fn parse_roundtrips_and_accepts_aliases() {
        for r in [
            UserRole::Viewer,
            UserRole::Editor,
            UserRole::Admin,
            UserRole::Owner,
        ] {
            assert_eq!(UserRole::parse(r.as_str()), Some(r));
        }
        assert_eq!(UserRole::parse("Administrator"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("read-only"), Some(UserRole::Viewer));
        assert_eq!(UserRole::parse("  CONTRIBUTOR "), Some(UserRole::Editor));
        assert_eq!(UserRole::parse("superuser"), None);
    }

    #[test]
    fn assignment_rules_prevent_privilege_escalation() {
        // Owner can grant anything.
        assert!(UserRole::Owner.can_assign(UserRole::Owner));
        assert!(UserRole::Owner.can_assign(UserRole::Admin));
        assert!(UserRole::Owner.can_assign(UserRole::Viewer));
        // Admin can only grant below Admin — cannot mint Admins or Owners.
        assert!(UserRole::Admin.can_assign(UserRole::Editor));
        assert!(UserRole::Admin.can_assign(UserRole::Viewer));
        assert!(!UserRole::Admin.can_assign(UserRole::Admin));
        assert!(!UserRole::Admin.can_assign(UserRole::Owner));
        // Editor/Viewer can assign nothing.
        assert!(!UserRole::Editor.can_assign(UserRole::Viewer));
        assert!(!UserRole::Viewer.can_assign(UserRole::Viewer));
    }

    #[test]
    fn serde_uses_lowercase_wire_form() {
        let json = serde_json::to_string(&UserRole::Admin).unwrap();
        assert_eq!(json, "\"admin\"");
        let parsed: UserRole = serde_json::from_str("\"owner\"").unwrap();
        assert_eq!(parsed, UserRole::Owner);
    }

    #[test]
    fn default_authenticated_is_editor() {
        assert_eq!(UserRole::default_authenticated(), UserRole::Editor);
    }
}
