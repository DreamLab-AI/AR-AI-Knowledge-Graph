use crate::services::nostr_service::NostrService;
use actix_web::{HttpRequest, HttpResponse};
use log::warn;
use tracing::{debug, info};
use uuid::Uuid;

/// Scoped permission levels for RBAC.
///
/// The hierarchy (from least to most privileged):
///   ReadOnly < WriteGraph < WriteSettings < Admin
///
/// Legacy mappings for backward compatibility:
///   - `Authenticated` maps to `ReadOnly + WriteGraph`
///   - `PowerUser` maps to `Admin`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessLevel {
    /// Legacy: any authenticated user (maps to ReadOnly + WriteGraph)
    Authenticated,
    /// Legacy: power user (maps to Admin)
    PowerUser,
    /// Can read graph data and settings, no mutations
    ReadOnly,
    /// Can mutate graph data (create/update/delete nodes and edges)
    WriteGraph,
    /// Can modify application settings
    WriteSettings,
    /// Full administrative access (includes all permissions)
    Admin,
}

impl AccessLevel {
    /// Check whether this access level satisfies the `required` permission.
    ///
    /// The mapping is:
    /// - `ReadOnly`: satisfied by ReadOnly, WriteGraph, WriteSettings, Admin, Authenticated, PowerUser
    /// - `WriteGraph`: satisfied by WriteGraph, Admin, Authenticated, PowerUser
    /// - `WriteSettings`: satisfied by WriteSettings, Admin, PowerUser
    /// - `Admin`: satisfied by Admin, PowerUser
    /// - `Authenticated`: satisfied by Authenticated, PowerUser, WriteGraph, WriteSettings, Admin, ReadOnly
    /// - `PowerUser`: satisfied by PowerUser, Admin
    pub fn has_permission(&self, required: &AccessLevel) -> bool {
        use AccessLevel::*;
        match required {
            ReadOnly => true,      // every authenticated level can read
            Authenticated => true, // same as ReadOnly for permission checks
            WriteGraph => matches!(self, WriteGraph | Admin | Authenticated | PowerUser),
            WriteSettings => matches!(self, WriteSettings | Admin | PowerUser),
            Admin => matches!(self, Admin | PowerUser),
            PowerUser => matches!(self, Admin | PowerUser),
        }
    }
}

pub async fn verify_access(
    req: &HttpRequest,
    nostr_service: &NostrService,
    required_level: AccessLevel,
) -> Result<String, HttpResponse> {
    let request_id = req
        .headers()
        .get("X-Request-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&Uuid::new_v4().to_string())
        .to_string();

    // --- Dev bypass (dev builds only) ---
    #[cfg(any(debug_assertions, feature = "dev-auth"))]
    {
        if let Some(auth_value) = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
        {
            if auth_value == "Bearer dev-session-token" {
                debug!("dev-auth: Bearer dev-session-token accepted in middleware (dev build)");
                let pubkey = req
                    .headers()
                    .get("X-Nostr-Pubkey")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("dev-user")
                    .to_string();
                return Ok(pubkey);
            }
        }
    }

    // --- NIP-98 Schnorr auth (primary path) ---
    if let Some(auth_value) = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
    {
        if auth_value.starts_with("Nostr ") {
            // Behind a TLS-terminating proxy, connection_info returns internal
            // scheme/host; prefer X-Forwarded-* headers from the proxy.
            let conn_info = req.connection_info();
            let scheme = req
                .headers()
                .get("X-Forwarded-Proto")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_else(|| conn_info.scheme());
            let host = req
                .headers()
                .get("X-Forwarded-Host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_else(|| conn_info.host());
            let url = format!(
                "{}://{}{}",
                scheme,
                host,
                req.uri()
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/")
            );
            let method = req.method().as_str();

            match nostr_service
                .verify_nip98_auth(auth_value, &url, method, None)
                .await
            {
                Ok(user) => {
                    info!(
                        request_id = %request_id,
                        pubkey = %user.pubkey,
                        "NIP-98 auth successful"
                    );
                    // Determine the user's effective access level
                    let user_level = if user.is_power_user {
                        AccessLevel::Admin
                    } else {
                        AccessLevel::Authenticated
                    };
                    if user_level.has_permission(&required_level) {
                        return Ok(user.pubkey);
                    } else {
                        warn!(
                            "User {} with level {:?} lacks required {:?}",
                            user.pubkey, user_level, required_level
                        );
                        return Err(HttpResponse::Forbidden()
                            .body("Insufficient permissions for this operation"));
                    }
                }
                Err(e) => {
                    warn!("[{}] NIP-98 validation failed: {}", request_id, e);
                    return Err(
                        HttpResponse::Unauthorized().body(format!("NIP-98 auth failed: {}", e))
                    );
                }
            }
        }
    }

    // --- Legacy path: X-Nostr-Pubkey + X-Nostr-Token ---
    let pubkey = match req.headers().get("X-Nostr-Pubkey") {
        Some(value) => value.to_str().unwrap_or("").to_string(),
        None => {
            warn!("Missing Nostr pubkey in request headers");
            debug!(
                request_id = %request_id,
                "Authentication failed - missing pubkey header"
            );
            return Err(HttpResponse::Forbidden().body("Authentication required"));
        }
    };

    let token = match req.headers().get("X-Nostr-Token") {
        Some(value) => value.to_str().unwrap_or("").to_string(),
        None => {
            warn!("Missing Nostr token in request headers");
            debug!(
                request_id = %request_id,
                has_pubkey = true,
                "Authentication failed - missing token header"
            );
            return Err(HttpResponse::Forbidden().body("Authentication required"));
        }
    };

    debug!(
        request_id = %request_id,
        has_pubkey = !pubkey.is_empty(),
        has_token = !token.is_empty(),
        pubkey_prefix = %&pubkey.chars().take(8).collect::<String>(),
        "Authentication headers extracted"
    );

    if !nostr_service.validate_session(&pubkey, &token).await {
        warn!("Invalid or expired session for user {}", pubkey);
        debug!(
            request_id = %request_id,
            pubkey = %pubkey,
            "Session validation failed"
        );
        return Err(HttpResponse::Unauthorized().body("Invalid or expired session"));
    }

    info!(
        request_id = %request_id,
        pubkey = %pubkey,
        "Session validated successfully"
    );

    // Determine the user's effective access level from their role
    let is_power = nostr_service.is_power_user(&pubkey).await;
    let user_level = if is_power {
        AccessLevel::Admin
    } else {
        AccessLevel::Authenticated
    };

    if user_level.has_permission(&required_level) {
        debug!(
            request_id = %request_id,
            pubkey = %pubkey,
            user_level = ?user_level,
            required_level = ?required_level,
            "Access granted"
        );
        Ok(pubkey)
    } else {
        warn!(
            "User {} with level {:?} lacks required {:?}",
            pubkey, user_level, required_level
        );
        debug!(
            request_id = %request_id,
            pubkey = %pubkey,
            "Access denied - insufficient permissions"
        );
        Err(HttpResponse::Forbidden().body("Insufficient permissions for this operation"))
    }
}

// Helper function for handlers that require power user access
pub async fn verify_power_user(
    req: &HttpRequest,
    nostr_service: &NostrService,
) -> Result<String, HttpResponse> {
    verify_access(req, nostr_service, AccessLevel::PowerUser).await
}

// Helper function for handlers that require authentication
pub async fn verify_authenticated(
    req: &HttpRequest,
    nostr_service: &NostrService,
) -> Result<String, HttpResponse> {
    verify_access(req, nostr_service, AccessLevel::Authenticated).await
}

// Helper function for handlers that require read-only access
pub async fn verify_read_only(
    req: &HttpRequest,
    nostr_service: &NostrService,
) -> Result<String, HttpResponse> {
    verify_access(req, nostr_service, AccessLevel::ReadOnly).await
}

// Helper function for handlers that require graph write access
pub async fn verify_write_graph(
    req: &HttpRequest,
    nostr_service: &NostrService,
) -> Result<String, HttpResponse> {
    verify_access(req, nostr_service, AccessLevel::WriteGraph).await
}

// Helper function for handlers that require settings write access
pub async fn verify_write_settings(
    req: &HttpRequest,
    nostr_service: &NostrService,
) -> Result<String, HttpResponse> {
    verify_access(req, nostr_service, AccessLevel::WriteSettings).await
}

// Helper function for handlers that require admin access
pub async fn verify_admin(
    req: &HttpRequest,
    nostr_service: &NostrService,
) -> Result<String, HttpResponse> {
    verify_access(req, nostr_service, AccessLevel::Admin).await
}

#[cfg(test)]
mod access_level_tests {
    use super::AccessLevel::*;

    /// S2 core invariant: a regular NIP-98 user resolves to `Authenticated`,
    /// which must satisfy WriteGraph but MUST NOT satisfy PowerUser/Admin. This
    /// is what makes the power_user escalations on destructive graph/ontology
    /// routes actually deny ordinary authenticated callers.
    #[test]
    fn authenticated_user_cannot_reach_power_user_or_admin() {
        assert!(Authenticated.has_permission(&ReadOnly));
        assert!(Authenticated.has_permission(&WriteGraph));
        assert!(Authenticated.has_permission(&Authenticated));
        // The escalation boundary:
        assert!(!Authenticated.has_permission(&PowerUser));
        assert!(!Authenticated.has_permission(&Admin));
        assert!(!Authenticated.has_permission(&WriteSettings));
    }

    /// A power user (mapped to Admin) satisfies every level, including the
    /// power_user-gated destructive routes.
    #[test]
    fn admin_power_user_satisfies_everything() {
        for required in [
            ReadOnly,
            WriteGraph,
            WriteSettings,
            Admin,
            PowerUser,
            Authenticated,
        ] {
            assert!(
                Admin.has_permission(&required),
                "Admin must satisfy {:?}",
                required
            );
            assert!(
                PowerUser.has_permission(&required),
                "PowerUser must satisfy {:?}",
                required
            );
        }
    }

    #[test]
    fn read_only_cannot_write() {
        assert!(ReadOnly.has_permission(&ReadOnly));
        assert!(!ReadOnly.has_permission(&WriteGraph));
        assert!(!ReadOnly.has_permission(&WriteSettings));
        assert!(!ReadOnly.has_permission(&Admin));
        assert!(!ReadOnly.has_permission(&PowerUser));
    }
}
