//! RES-a LivenessHarness core (PRD-023 WP-11, CANARY-VC-RESA-KG).
//!
//! Exercises the durable canary store + the harness gauge/transition logic over
//! the crate's public API: register (idempotent), observe (fire), the SHA/30-day
//! staleness rule, NotFound on an unregistered canary, and the KG watchdog gauge
//! firing CANARY-VC-RESA-KG on every state transition. No synthetic probe stands
//! in for a fire (DDD invariant 5): a fire is only ever recorded from an observed
//! transition.

use std::sync::Arc;

use visionclaw_server::adapters::{CanaryRegistration, CanaryStoreError, SqliteCanaryRepository};
use visionclaw_server::services::liveness_harness::{
    LivenessHarness, CANARY_COM18_INTERV, CANARY_KG, CANARY_M4_RAY,
};

const WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

async fn temp_repo() -> SqliteCanaryRepository {
    let dir = std::env::temp_dir().join(format!("canary-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!(
        "liveness-{}.sqlite3",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    SqliteCanaryRepository::open(&path)
        .await
        .expect("open canary repo")
}

fn reg(id: &str, kind: &str, sha: &str) -> CanaryRegistration {
    CanaryRegistration {
        canary_id: id.to_string(),
        description: "test wire".to_string(),
        kind: kind.to_string(),
        owner_repo: "visionclaw".to_string(),
        wave: Some("P0".to_string()),
        sha_at_registration: sha.to_string(),
        registered_at_ms: 1_000_000,
    }
}

#[tokio::test]
async fn register_observe_status_and_staleness_rule() {
    let repo = temp_repo().await;
    repo.register(&reg("CANARY-X", "standing", "shaA"))
        .await
        .unwrap();

    // Before any fire: armed, not fired, zero observations.
    let s = repo.all_status("shaA", 2_000_000, WINDOW_MS).await.unwrap();
    assert_eq!(s.len(), 1);
    assert!(s[0].armed && !s[0].fired);
    assert_eq!(s[0].observation_count, 0);
    assert_eq!(s[0].last_fired_at, None);

    // Observe (fire) at shaA, t = 2_000_000.
    repo.observe("CANARY-X", "live traffic", "shaA", 2_000_000)
        .await
        .unwrap();

    // Same sha, within window → fired.
    let s = repo.all_status("shaA", 2_000_050, WINDOW_MS).await.unwrap();
    assert!(s[0].fired && !s[0].armed);
    assert_eq!(s[0].observation_count, 1);
    assert_eq!(s[0].last_fired_at, Some(2_000_000));

    // SHA advanced → the fire is bound to shaA, so it re-arms at shaB.
    let s = repo.all_status("shaB", 2_000_050, WINDOW_MS).await.unwrap();
    assert!(
        s[0].armed && !s[0].fired,
        "a fire at an older sha must not count toward closure at the new sha"
    );

    // Beyond the 30-day window → re-arms even at the matching sha.
    let s = repo
        .all_status("shaA", 2_000_000 + 40 * DAY_MS, WINDOW_MS)
        .await
        .unwrap();
    assert!(
        s[0].armed && !s[0].fired,
        "a stale fire beyond the freshness window re-arms"
    );
}

#[tokio::test]
async fn observe_unknown_canary_is_not_found() {
    let repo = temp_repo().await;
    let err = repo
        .observe("no-such-canary", "x", "sha", 1)
        .await
        .unwrap_err();
    assert!(matches!(err, CanaryStoreError::NotFound(_)));
}

#[tokio::test]
async fn register_is_idempotent_preserving_registration_sha() {
    let repo = temp_repo().await;
    repo.register(&reg("CANARY-Y", "one-shot", "sha-first"))
        .await
        .unwrap();
    // Re-register with a changed kind + sha: identity fields preserved, descriptor
    // fields refreshed — the exact posture start-up re-seeding relies on.
    repo.register(&reg("CANARY-Y", "standing", "sha-second"))
        .await
        .unwrap();
    let s = repo.all_status("x", 3_000_000, WINDOW_MS).await.unwrap();
    assert_eq!(s.len(), 1, "idempotent: still one row");
    assert_eq!(
        s[0].sha_at_registration, "sha-first",
        "registration sha preserved across re-registration"
    );
    assert_eq!(s[0].kind, "standing", "descriptor fields refreshed");
}

#[tokio::test]
async fn kg_watchdog_gauge_transitions_fire_the_canary() {
    let repo = Arc::new(temp_repo().await);
    let harness = LivenessHarness::new(repo);
    harness.seed_p0_canaries().await.unwrap();

    // Gauge starts unknown; the KG canary is registered but unfired.
    assert_eq!(harness.kg_backend_up(), None);

    // unknown -> up is a transition: fires once and proves the watchdog is live.
    assert!(harness.record_kg_state(true).await);
    assert_eq!(harness.kg_backend_up(), Some(true));

    // up -> up is not a transition: no new fire.
    assert!(!harness.record_kg_state(true).await);

    // up -> down (simulated backend loss) fires again rather than failing open.
    assert!(harness.record_kg_state(false).await);
    assert_eq!(harness.kg_backend_up(), Some(false));

    // Status shows RESA-KG fired at the current sha with >= 2 observations.
    let status = harness.status().await.unwrap();
    let kg = status
        .iter()
        .find(|c| c.canary_id == CANARY_KG)
        .expect("RESA-KG seeded at startup");
    assert!(
        kg.observation_count >= 2,
        "at least two transitions recorded, got {}",
        kg.observation_count
    );
    assert!(
        kg.fired && !kg.armed,
        "RESA-KG fired at the current sha within the window"
    );
}

#[tokio::test]
async fn p2_seed_registers_the_mr_copresence_canaries_armed() {
    // WP-9 (M4 / COM-18 / M2): the P2 seed must register the two MR canaries so
    // the xr-runtime sidecar can fire them over the shared observe path. They are
    // armed-not-fired at boot — no synthetic probe stands in for the on-device
    // fire (DDD invariant 5).
    let repo = Arc::new(temp_repo().await);
    let harness = LivenessHarness::new(repo);
    harness.seed_p2_canaries().await.unwrap();

    let status = harness.status().await.unwrap();

    let ray = status
        .iter()
        .find(|c| c.canary_id == CANARY_M4_RAY)
        .expect("CANARY-VC-M4-RAY seeded by seed_p2_canaries");
    assert_eq!(ray.kind, "one-shot", "M4 ray is a one-shot resolution proof");
    assert_eq!(ray.wave.as_deref(), Some("P2"));
    assert!(ray.armed && !ray.fired, "armed until the sidecar fires it live");
    assert_eq!(ray.observation_count, 0);

    let interv = status
        .iter()
        .find(|c| c.canary_id == CANARY_COM18_INTERV)
        .expect("CANARY-VC-COM18-INTERV seeded by seed_p2_canaries");
    assert_eq!(
        interv.kind, "standing",
        "COM-18 intervention is a standing governance monitor"
    );
    assert_eq!(interv.wave.as_deref(), Some("P2"));
    assert!(interv.armed && !interv.fired);
}
