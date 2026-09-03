//! Security validation probes.
//!
//! Port of `tests/integration/security_validation_test.py`. The assertions are
//! carried over unchanged; what has gone is the `requests` + `cryptography` +
//! `psutil` dependency set and the unused HMAC token generator the Python
//! `SecurityTester` defined but never called.

use serde_json::json;
use visionclaw_integration_tests::{payloads, require_server, Harness};

/// Was an injection attempt rejected? A transport failure counts as rejection —
/// the payload did not reach anything that could act on it.
async fn injection_blocked(h: &Harness, path: &str, payload: &str) -> bool {
    match h.post_json(path, &json!({ "data": payload })).await {
        Some(response) => {
            let status = response.status().as_u16();
            status == 400 || status == 403
        }
        None => true,
    }
}

#[tokio::test]
async fn sql_injection_is_rejected() {
    let h = require_server!();
    for payload in payloads::SQL {
        assert!(
            injection_blocked(&h, "/api/query", payload).await,
            "SQL injection was not blocked: {payload}"
        );
    }
}

#[tokio::test]
async fn xss_payloads_are_not_reflected() {
    let h = require_server!();
    for payload in payloads::XSS {
        let Some(response) = h.post_json("/api/content", &json!({ "data": payload })).await else {
            continue;
        };
        if response.status() != reqwest::StatusCode::OK {
            continue;
        }
        let body = response.text().await.unwrap_or_default();
        assert!(!body.contains("<script>"), "a 200 response reflected a script tag: {payload}");
        assert!(!body.contains("javascript:"), "a 200 response reflected a js: URL: {payload}");
    }
}

#[tokio::test]
async fn command_injection_is_rejected() {
    let h = require_server!();
    for payload in payloads::COMMAND {
        assert!(
            injection_blocked(&h, "/api/process", payload).await,
            "command injection was not blocked: {payload}"
        );
    }
}

#[tokio::test]
async fn path_traversal_is_rejected() {
    let h = require_server!();
    for payload in payloads::PATH_TRAVERSAL {
        let Some(response) = h
            .http()
            .get(h.url("/api/file"))
            .query(&[("path", payload)])
            .send()
            .await
            .ok()
        else {
            continue; // transport failure is a rejection
        };
        let status = response.status().as_u16();
        assert!(
            matches!(status, 400 | 403 | 404),
            "path traversal returned {status} rather than a rejection: {payload}"
        );
    }
}

#[tokio::test]
async fn rate_limiting_is_enforced() {
    let h = require_server!();

    let mut sent = 0usize;
    let mut limited = false;
    for _ in 0..50 {
        sent += 1;
        match h.get("/api/data").await {
            Some(response) if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS => {
                limited = true;
                break;
            }
            Some(_) => {}
            None => break,
        }
    }

    assert!(limited, "no 429 after {sent} requests — rate limiting is not enforced");
    assert!(sent < 50, "the limiter allowed all 50 requests through");
}

#[tokio::test]
async fn protected_endpoints_demand_authentication() {
    let h = require_server!();
    for path in ["/api/admin", "/api/user/profile", "/api/settings", "/api/secure-data"] {
        let Some(response) = h.get(path).await else { continue };
        let status = response.status().as_u16();
        assert!(matches!(status, 401 | 403), "{path} answered {status}, so it is unprotected");
    }
}

#[tokio::test]
async fn authentication_cannot_be_bypassed() {
    let h = require_server!();

    // Each of these must be refused. The final entry is an `alg: none` JWT.
    let attempts: [Option<&str>; 5] = [
        None,
        Some(""),
        Some("InvalidToken"),
        Some("' OR '1'='1"),
        Some(
            "Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJub25lIn0.\
             eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.",
        ),
    ];

    for attempt in attempts {
        let mut request = h.http().get(h.url("/api/secure"));
        if let Some(value) = attempt {
            request = request.header("Authorization", value);
        }
        let Some(response) = request.send().await.ok() else { continue };
        let status = response.status().as_u16();
        assert!(
            matches!(status, 401 | 403),
            "auth bypass succeeded with Authorization={attempt:?} (status {status})"
        );
    }
}

#[tokio::test]
async fn malformed_input_is_rejected() {
    let h = require_server!();

    let invalid = [
        json!({ "data": null }),
        json!({ "data": "x".repeat(10_000) }),
        json!({ "data": { "nested": { "too": { "deep": { "for": { "safety": "test" } } } } } }),
        json!({ "data": vec!["a"; 1000] }),
        json!({ "number": "not-a-number" }),
        json!({ "email": "invalid-email" }),
        json!({ "url": "javascript:alert(1)" }),
    ];

    for payload in &invalid {
        let Some(response) = h.post_json("/api/validate", payload).await else { continue };
        let status = response.status().as_u16();
        assert!(matches!(status, 400 | 422), "invalid input accepted with {status}: {payload}");
    }
}

#[tokio::test]
async fn security_headers_hold_their_documented_values() {
    let h = require_server!();
    let Some(response) = h.get("/").await else {
        eprintln!("SKIP: the root path did not answer.");
        return;
    };
    let headers = response.headers();

    // Present-and-wrong is a failure; absent is a warning, exactly as the
    // Python suite treated it — these are hardening headers, not a contract.
    if let Some(value) = headers.get("X-Content-Type-Options") {
        assert_eq!(value, "nosniff", "X-Content-Type-Options has a non-standard value");
    }
    if let Some(value) = headers.get("X-Frame-Options") {
        let value = value.to_str().unwrap_or_default();
        assert!(
            value == "DENY" || value == "SAMEORIGIN",
            "X-Frame-Options is neither DENY nor SAMEORIGIN: {value}"
        );
    }
    if let Some(value) = headers.get("X-XSS-Protection") {
        assert_eq!(value, "1; mode=block", "X-XSS-Protection has a non-standard value");
    }

    for header in ["X-Content-Type-Options", "X-Frame-Options", "Content-Security-Policy"] {
        if !headers.contains_key(header) {
            eprintln!("WARN: security header absent: {header}");
        }
    }
}

#[tokio::test]
async fn oversized_payloads_are_refused() {
    let h = require_server!();
    let payload = json!({ "data": "x".repeat(10 * 1024 * 1024) });

    let Some(response) = h.post_json("/api/data", &payload).await else {
        return; // the connection was cut — that is a refusal
    };
    let status = response.status().as_u16();
    assert!(matches!(status, 400 | 413), "a 10 MiB body was accepted with {status}");
}

#[tokio::test]
async fn connection_flooding_is_throttled() {
    let h = require_server!();

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..100 {
        let h = h.clone();
        tasks.spawn(async move {
            match h.get("/api/data").await {
                Some(response) => response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS,
                None => false,
            }
        });
    }

    let mut blocked = 0usize;
    while let Some(outcome) = tasks.join_next().await {
        if !outcome.unwrap_or(false) {
            blocked += 1;
        }
    }

    assert!(blocked > 0, "100 concurrent requests all succeeded — no flood protection");
}

#[tokio::test]
async fn debug_endpoints_do_not_leak_secrets() {
    let h = require_server!();

    for path in
        ["/api/config", "/api/environment", "/api/debug", "/api/status", "/.env", "/config.json"]
    {
        let Some(response) = h.get(path).await else { continue };
        if response.status() != reqwest::StatusCode::OK {
            continue;
        }
        let body = response.text().await.unwrap_or_default().to_lowercase();
        for marker in payloads::SECRET_MARKERS {
            assert!(!body.contains(marker), "{path} returned 200 exposing `{marker}`");
        }
    }
}

#[tokio::test]
async fn cors_does_not_admit_every_origin() {
    let h = require_server!();

    let Some(response) = h
        .http()
        .request(reqwest::Method::OPTIONS, h.url("/api/data"))
        .header("Origin", "http://evil.com")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Content-Type")
        .send()
        .await
        .ok()
    else {
        eprintln!("SKIP: the CORS preflight did not answer.");
        return;
    };

    if let Some(origin) = response.headers().get("Access-Control-Allow-Origin") {
        assert_ne!(origin, "*", "CORS echoes a wildcard origin on /api/data");
    }
}
