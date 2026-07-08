// src/services/liveness_harness.rs
//! LivenessHarness — the sprint-wide live-traffic observer (RES-a, ADR-130 D3).
//!
//! A canary registers a wire; the harness records a `CanaryFired` only when real
//! traffic crosses that wire. It is NOT a synthetic prober — a green ping never
//! stands in for an observation (DDD invariant 5). The harness backs three HTTP
//! surfaces (`register`/`observe`/`status`, see
//! [`crate::handlers::liveness_harness_handler`]) and drives the KG-backend
//! watchdog below.
//!
//! ## KG watchdog
//!
//! VisionClaw's own server IS the KG backend (port 4000). [`run_kg_watchdog`]
//! is a tokio interval task that self-polls `/api/health` and drives the
//! `kg_backend_up` gauge held on the harness ([`LivenessHarness::kg_backend_up`]).
//! Every state transition — including the first `unknown → up`, which proves the
//! watchdog is live, and any later `up → down` on backend loss — fires
//! [`CANARY_KG`]. Fail-open: a poll failure marks the backend down (and fires),
//! it never panics or blocks the runtime.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info, warn};

use crate::adapters::sqlite_canary_repository::{
    CanaryRegistration, CanaryStatus, Result as CanaryResult, SqliteCanaryRepository,
};

/// The KG-backend liveness canary the watchdog fires on gauge transitions.
pub const CANARY_KG: &str = "CANARY-VC-RESA-KG";

/// The broker case-queue round-trip canary (REC-2 / D3). Fires from the
/// enrichment-decision path when a queued case reaches a decision over live
/// traffic (`broker:new_case` → `broker:case_decided`).
pub const CANARY_REC2_CASE: &str = "CANARY-VC-REC2-CASE";

/// Freshness window for the staleness rule (WP-11): a fire older than this
/// re-arms its canary. 30 days per the canon default.
pub const FRESHNESS_WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// The P0-wave canaries this repository seeds at start-up (PRD-023 canary
/// table). Tuple: `(canary_id, description, kind, wave)`. Registration is
/// idempotent, so re-seeding on every boot is safe.
pub const P0_CANARIES: &[(&str, &str, &str, &str)] = &[
    (
        "CANARY-VC-COM14-DID",
        "Selected node addressable by a verified did:nostr (Schnorr challenge at selection)",
        "standing",
        "P0",
    ),
    (
        CANARY_REC2_CASE,
        "broker:new_case then broker:case_decided on the multiplexed graph socket",
        "standing",
        "P0",
    ),
    (
        "CANARY-VC-D5-WS",
        "WS status dot transitions to disconnected on a real socket drop",
        "one-shot",
        "P0",
    ),
    (
        "CANARY-VC-M1-HUD",
        "Godot avatar renders a verified DID badge in an xr-runtime session",
        "standing",
        "P0",
    ),
    (
        CANARY_KG,
        "kg_backend_up gauge transition on the watchdog self-poll of /api/health",
        "standing",
        "P0",
    ),
    (
        "CANARY-VC-REC1-ROUTE",
        "Route dump shows no unauthenticated ontology ingest route (auth gates hold)",
        "one-shot",
        "P0",
    ),
];

/// The governed-voice-loop canary (COM-15 / V1 / D6 / M5, PRD-023 WP-5). Fires
/// on the live end-to-end: a spoken command bound to the selected agent's
/// `did:nostr` → a signed 31402 accepted by agentbox `/v1/voice-intent` → a
/// Kokoro TTS acknowledgement. Standing (P1).
pub const CANARY_COM15_PTT: &str = "CANARY-VC-COM15-PTT";

/// The steering-surface canary (D2, PRD-023 WP-3). Fires when a steer action
/// (`/bots/submit-task` or `/bots/interrupt`) is invoked from a mounted
/// per-agent panel — the route handler observes it as live traffic, so a fire
/// means node selection opened a working steering control. Standing (P1).
pub const CANARY_D2_STEER: &str = "CANARY-VC-D2-STEER";

/// The swarm-observability canary (D8, PRD-023 WP-3). Fires when the aggregate
/// swarm dashboard mounts with live poll data — the client observes it over
/// `POST /api/canary/observe/{id}`. One-shot (P1).
pub const CANARY_D8_OBS: &str = "CANARY-VC-D8-OBS";

/// The P1-wave canaries this repository seeds at start-up (PRD-023 canary
/// table). Idempotent, so re-seeding on every boot is safe. Kept separate from
/// [`P0_CANARIES`] so each wave's rows stay legible; more P1 rows land as their
/// items close.
pub const P1_CANARIES: &[(&str, &str, &str, &str)] = &[
    (
        CANARY_COM15_PTT,
        "Spoken command bound to the selected agent → signed 31402 accepted by \
         /v1/voice-intent → Kokoro TTS acknowledgement",
        "standing",
        "P1",
    ),
    (
        CANARY_D2_STEER,
        "Steer action (/bots/submit-task or /bots/interrupt) invoked from a \
         mounted per-agent panel",
        "standing",
        "P1",
    ),
    (
        CANARY_D8_OBS,
        "Aggregate swarm-observability dashboard mounted with live poll data",
        "one-shot",
        "P1",
    ),
];

// KG gauge tri-state (an atomic 3-valued gauge, mirroring the AtomicUsize
// counter idiom used for `active_connections`).
const KG_UNKNOWN: u8 = 0;
const KG_UP: u8 = 1;
const KG_DOWN: u8 = 2;

fn kg_label(state: u8) -> &'static str {
    match state {
        KG_UP => "up",
        KG_DOWN => "down",
        _ => "unknown",
    }
}

/// The git SHA the running binary is bound to. Runtime `VISIONCLAW_GIT_SHA`
/// overrides the build-time value embedded by `build.rs`; falls back to
/// `"unknown"`. Used to bind a canary fire to the commit it fired at.
pub fn current_sha() -> String {
    std::env::var("VISIONCLAW_GIT_SHA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| option_env!("VISIONCLAW_GIT_SHA").map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The central live-traffic observer. Cheap to clone via `Arc`; the KG gauge is
/// a lock-free atomic so the watchdog and the status handler share it directly.
pub struct LivenessHarness {
    repo: Arc<SqliteCanaryRepository>,
    kg_state: AtomicU8,
}

impl LivenessHarness {
    /// Build a harness over an opened canary store.
    pub fn new(repo: Arc<SqliteCanaryRepository>) -> Self {
        Self {
            repo,
            kg_state: AtomicU8::new(KG_UNKNOWN),
        }
    }

    /// Register (idempotently) a canary declaration from any repository.
    pub async fn register(&self, reg: &CanaryRegistration) -> CanaryResult<()> {
        self.repo.register(reg).await
    }

    /// Record a fire on a registered canary from observed live traffic. Binds
    /// the fire to [`current_sha`] and the wall clock.
    pub async fn observe(&self, canary_id: &str, evidence: &str) -> CanaryResult<i64> {
        self.repo
            .observe(canary_id, evidence, &current_sha(), now_ms())
            .await
    }

    /// Per-canary status applying the 30-day/SHA staleness rule.
    pub async fn status(&self) -> CanaryResult<Vec<CanaryStatus>> {
        self.repo
            .all_status(&current_sha(), now_ms(), FRESHNESS_WINDOW_MS)
            .await
    }

    /// Idempotently seed the P0-wave canaries so the watchdog's target exists
    /// and the harness is immediately queryable at boot.
    pub async fn seed_p0_canaries(&self) -> CanaryResult<()> {
        let sha = current_sha();
        let at = now_ms();
        for (id, description, kind, wave) in P0_CANARIES {
            self.register(&CanaryRegistration {
                canary_id: (*id).to_string(),
                description: (*description).to_string(),
                kind: (*kind).to_string(),
                owner_repo: "visionclaw".to_string(),
                wave: Some((*wave).to_string()),
                sha_at_registration: sha.clone(),
                registered_at_ms: at,
            })
            .await?;
        }
        info!(
            "[liveness] seeded {} P0 canaries at sha={}",
            P0_CANARIES.len(),
            sha
        );
        Ok(())
    }

    /// Idempotently seed the P1-wave canaries (COM-15 governed voice loop, and
    /// any later P1 rows). Registration is idempotent; a live fire is recorded
    /// separately via [`Self::observe`] on the standing wire.
    pub async fn seed_p1_canaries(&self) -> CanaryResult<()> {
        let sha = current_sha();
        let at = now_ms();
        for (id, description, kind, wave) in P1_CANARIES {
            self.register(&CanaryRegistration {
                canary_id: (*id).to_string(),
                description: (*description).to_string(),
                kind: (*kind).to_string(),
                owner_repo: "visionclaw".to_string(),
                wave: Some((*wave).to_string()),
                sha_at_registration: sha.clone(),
                registered_at_ms: at,
            })
            .await?;
        }
        info!(
            "[liveness] seeded {} P1 canaries at sha={}",
            P1_CANARIES.len(),
            sha
        );
        Ok(())
    }

    /// The `kg_backend_up` gauge: `None` until the first poll, then `Some(true)`
    /// / `Some(false)`.
    pub fn kg_backend_up(&self) -> Option<bool> {
        match self.kg_state.load(Ordering::SeqCst) {
            KG_UP => Some(true),
            KG_DOWN => Some(false),
            _ => None,
        }
    }

    /// Drive the gauge from a watchdog observation. On a state TRANSITION only,
    /// fires [`CANARY_KG`] with the transition as evidence and returns `true`.
    /// A repeat of the current state is a no-op (returns `false`) — the canary
    /// fires on observed change, not on every tick.
    pub async fn record_kg_state(&self, up: bool) -> bool {
        let new = if up { KG_UP } else { KG_DOWN };
        let prev = self.kg_state.swap(new, Ordering::SeqCst);
        if prev == new {
            return false;
        }
        let evidence = format!(
            "kg_backend_up: {} -> {} (watchdog self-poll /api/health)",
            kg_label(prev),
            kg_label(new)
        );
        if let Err(e) = self.observe(CANARY_KG, &evidence).await {
            warn!("[liveness] failed to record {CANARY_KG} fire: {e}");
        }
        info!("[liveness] {evidence}");
        true
    }
}

/// Run the KG-backend watchdog: self-poll `/api/health` every `period` and
/// drive the `kg_backend_up` gauge, firing [`CANARY_KG`] on every transition.
/// Never returns under normal operation; fail-open on every poll error.
pub async fn run_kg_watchdog(harness: Arc<LivenessHarness>, self_url: String, period: Duration) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("[liveness] KG watchdog HTTP client build failed, watchdog disabled: {e}");
            return;
        }
    };
    info!(
        "[liveness] KG watchdog self-polling {}/api/health every {:?}",
        self_url.trim_end_matches('/'),
        period
    );
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let up = probe_kg(&client, &self_url).await;
        harness.record_kg_state(up).await;
    }
}

/// One health probe. `true` iff `/api/health` answers 2xx and its `status`
/// field is not `"unhealthy"`. A connection failure, timeout or non-2xx is
/// backend loss (`false`).
async fn probe_kg(client: &reqwest::Client, base_url: &str) -> bool {
    let url = format!("{}/api/health", base_url.trim_end_matches('/'));
    match client.get(&url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return false;
            }
            match resp.json::<serde_json::Value>().await {
                Ok(v) => v
                    .get("status")
                    .and_then(|s| s.as_str())
                    .map(|s| s != "unhealthy")
                    .unwrap_or(true),
                // Reachable but unparseable body → the endpoint answered, treat as up.
                Err(_) => true,
            }
        }
        Err(_) => false,
    }
}
