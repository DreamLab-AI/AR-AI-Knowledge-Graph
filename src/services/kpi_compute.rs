//! KpiComputeService — the REC-4 four-KPI compute engine (ADR-043 resurrection,
//! ADR-130 Decision 5).
//!
//! ADR-043 named four organisational KPIs. Two compute now, from sources that
//! already exist, without new instrumentation at the emit site:
//!
//!   * **Augmentation Ratio** — agent-action volume ÷ ACSP escalation volume.
//!     Numerator: the count of `/wss/agent-events` envelopes observed in the
//!     rolling window (the passive hub tap in [`run_agent_event_tap`]).
//!     Denominator: the count of broker/enrichment decisions in the window (the
//!     ACSP escalation store, `enrichment_decisions`). A higher ratio means more
//!     autonomous agent work per human/broker escalation.
//!
//!   * **Trust Variance** — the dispersion of decision outcomes over the rolling
//!     window, as a Gini-Simpson index (`1 − Σ pᵢ²`) of the outcome categories,
//!     normalised to `[0, 1]`. Low dispersion ⇒ decisions cluster on one outcome
//!     (stable trust); high dispersion ⇒ outcomes scatter (volatile trust). This
//!     is the v1 outcome-category proxy for ADR-043's "rolling variance in
//!     decision quality / override rates".
//!
//! The other two (Mesh Velocity, HITL Precision) have no source event yet — the
//! REC-10 insight loop and the WP-4 case-queue HITL flag supply them later — so
//! the dashboard renders them honestly as "awaiting data source", never faked.
//!
//! Each compute persists a [`KpiSnapshotRow`] with its lineage (WP-8 AC3) and
//! fires `CANARY-VC-REC4-KPI` as observed live traffic.

use std::collections::HashMap;
use std::sync::Arc;

use log::{debug, warn};
use serde::Serialize;

use crate::adapters::sqlite_enrichment_repository::SqliteEnrichmentRepository;
use crate::adapters::sqlite_kpi_repository::{NewKpiSnapshot, SqliteKpiRepository};
use crate::services::liveness_harness::{LivenessHarness, CANARY_REC4_KPI};

/// The rolling window for both computed KPIs: 30 days (ADR-043 / the "30-day
/// rolling" Trust-Variance spec).
pub const KPI_WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Sample size at which a KPI reaches full confidence. Below it, confidence
/// scales linearly with the sample count so a value computed from three events
/// is not presented with the authority of one computed from three hundred.
pub const FULL_CONFIDENCE_SAMPLE: u64 = 30;

/// The canonical KPI ids (the `kpi` column and the dashboard tile keys).
pub const KPI_AUGMENTATION_RATIO: &str = "augmentation_ratio";
pub const KPI_TRUST_VARIANCE: &str = "trust_variance";
pub const KPI_MESH_VELOCITY: &str = "mesh_velocity";
pub const KPI_HITL_PRECISION: &str = "hitl_precision";

/// Cap on per-decision lineage rows written for one Trust-Variance snapshot, so a
/// pathologically large window cannot bloat the lineage table. The aggregate
/// per-outcome-category rows are always written in full.
const MAX_DECISION_LINEAGE_ROWS: usize = 1000;

// ---------------------------------------------------------------------------
// Pure computation (unit-tested against fixture rows)
// ---------------------------------------------------------------------------

/// Linear confidence in `[0, 1]` from a sample count.
pub fn sample_confidence(sample: u64) -> f64 {
    (sample as f64 / FULL_CONFIDENCE_SAMPLE as f64).min(1.0)
}

/// Augmentation Ratio = agent-action volume ÷ ACSP escalation volume.
///
/// Returns `(value, confidence)`. With zero escalations the ratio is undefined,
/// so it reports `(0.0, 0.0)` — a value with no confidence — rather than an
/// infinite or NaN number the dashboard would have to special-case.
pub fn augmentation_ratio(agent_volume: u64, escalation_volume: u64) -> (f64, f64) {
    if escalation_volume == 0 {
        return (0.0, 0.0);
    }
    let value = agent_volume as f64 / escalation_volume as f64;
    let sample = agent_volume.saturating_add(escalation_volume);
    (value, sample_confidence(sample))
}

/// Trust Variance = normalised Gini-Simpson dispersion of decision outcomes.
///
/// Returns `(value, confidence, sample_count)`. `value` is `0.0` when every
/// decision shares one outcome (no dispersion) and `1.0` at maximum spread
/// across the observed categories. Normalisation divides the raw Gini-Simpson
/// index by its theoretical maximum `1 − 1/k` (k = distinct outcomes) so the
/// figure is comparable regardless of how many categories appear.
pub fn trust_variance(outcomes: &[String]) -> (f64, f64, u64) {
    let n = outcomes.len();
    if n == 0 {
        return (0.0, 0.0, 0);
    }
    let mut counts: HashMap<&str, u64> = HashMap::new();
    for o in outcomes {
        *counts.entry(o.as_str()).or_insert(0) += 1;
    }
    let total = n as f64;
    let sum_sq: f64 = counts
        .values()
        .map(|c| {
            let p = *c as f64 / total;
            p * p
        })
        .sum();
    let gini = 1.0 - sum_sq;
    let k = counts.len();
    let normalised = if k <= 1 {
        0.0
    } else {
        gini / (1.0 - 1.0 / k as f64)
    };
    (normalised.clamp(0.0, 1.0), sample_confidence(n as u64), n as u64)
}

// ---------------------------------------------------------------------------
// Summary shape (the GET /api/kpi/summary payload)
// ---------------------------------------------------------------------------

/// One dashboard tile. `status` is `"computed"` for a live KPI or
/// `"awaiting_data_source"` for one with no source event yet — the latter
/// carries only the named source, never a fabricated value.
#[derive(Debug, Clone, Serialize)]
pub struct KpiTile {
    pub kpi: String,
    pub label: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numerator: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denominator: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_days: Option<i64>,
    /// The named source event stream. Always present — for computed KPIs it
    /// documents the derivation; for awaiting KPIs it names what is missing.
    pub source: &'static str,
}

impl KpiTile {
    fn awaiting(kpi: &str, label: &str, source: &'static str) -> Self {
        Self {
            kpi: kpi.to_string(),
            label: label.to_string(),
            status: "awaiting_data_source",
            value: None,
            confidence: None,
            unit: None,
            numerator: None,
            denominator: None,
            sample_count: None,
            snapshot_id: None,
            window_days: None,
            source,
        }
    }
}

/// The full four-KPI summary the dashboard renders.
#[derive(Debug, Clone, Serialize)]
pub struct KpiSummary {
    pub tiles: Vec<KpiTile>,
    pub computed_at_ms: i64,
    pub window_days: i64,
    pub sha: String,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Computes the two live KPIs from real source events, persists snapshots with
/// lineage, and reports the four-tile summary. Cheap to clone via `Arc`.
pub struct KpiComputeService {
    kpi_repo: Arc<SqliteKpiRepository>,
    enrichment_repo: Arc<SqliteEnrichmentRepository>,
    harness: Arc<LivenessHarness>,
}

impl KpiComputeService {
    pub fn new(
        kpi_repo: Arc<SqliteKpiRepository>,
        enrichment_repo: Arc<SqliteEnrichmentRepository>,
        harness: Arc<LivenessHarness>,
    ) -> Self {
        Self {
            kpi_repo,
            enrichment_repo,
            harness,
        }
    }

    fn now_ms() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    /// Compute both live KPIs over `[now - KPI_WINDOW_MS, now]` from real source
    /// events, persist a snapshot with lineage for each, fire `CANARY-VC-REC4-KPI`,
    /// and return the four-tile summary. This is the read path for
    /// `GET /api/kpi/summary`: a read computes fresh and persists, so the stored
    /// series always traces to the events that produced it.
    pub async fn compute_and_persist(&self) -> Result<KpiSummary, String> {
        let now = Self::now_ms();
        let window_start = now.saturating_sub(KPI_WINDOW_MS);
        let sha = crate::services::liveness_harness::current_sha();

        // --- source reads --------------------------------------------------
        let agent_volume = self
            .kpi_repo
            .count_agent_events_since(window_start)
            .await
            .map_err(|e| format!("agent-event volume read failed: {e}"))?
            as u64;

        let decisions = self
            .enrichment_repo
            .decisions_since(window_start)
            .await
            .map_err(|e| format!("decision read failed: {e}"))?;
        let escalation_volume = decisions.len() as u64;

        // --- Augmentation Ratio -------------------------------------------
        let (ar_value, ar_conf) = augmentation_ratio(agent_volume, escalation_volume);
        let ar_snapshot = NewKpiSnapshot {
            kpi: KPI_AUGMENTATION_RATIO.into(),
            value: ar_value,
            confidence: ar_conf,
            numerator: Some(agent_volume as f64),
            denominator: Some(escalation_volume as f64),
            sample_count: (agent_volume + escalation_volume) as i64,
            window_start_ms: window_start,
            window_end_ms: now,
            computed_at_ms: now,
            sha: sha.clone(),
        };
        let ar_lineage = vec![
            (
                "agent_event_volume".to_string(),
                "wss/agent-events window count".to_string(),
                Some(agent_volume as f64),
            ),
            (
                "acsp_escalation".to_string(),
                "enrichment_decisions window count".to_string(),
                Some(escalation_volume as f64),
            ),
        ];
        let ar_id = self
            .kpi_repo
            .insert_snapshot_with_lineage(&ar_snapshot, &ar_lineage)
            .await
            .map_err(|e| format!("augmentation-ratio persist failed: {e}"))?;

        // --- Trust Variance -----------------------------------------------
        let outcomes: Vec<String> = decisions.iter().map(|(o, _, _)| o.clone()).collect();
        let (tv_value, tv_conf, tv_sample) = trust_variance(&outcomes);

        // Lineage: one row per distinct outcome category (with its count) and one
        // row per contributing decision (its activity URN), so the value traces
        // back to the decision events (WP-8 AC3).
        let mut tv_lineage: Vec<(String, String, Option<f64>)> = Vec::new();
        let mut category_counts: HashMap<&str, f64> = HashMap::new();
        for (outcome, _, _) in &decisions {
            *category_counts.entry(outcome.as_str()).or_insert(0.0) += 1.0;
        }
        for (category, count) in &category_counts {
            tv_lineage.push((
                "outcome_category".to_string(),
                (*category).to_string(),
                Some(*count),
            ));
        }
        for (_, activity_urn, _) in decisions.iter().take(MAX_DECISION_LINEAGE_ROWS) {
            tv_lineage.push((
                "enrichment_decision".to_string(),
                activity_urn.clone(),
                Some(1.0),
            ));
        }
        let tv_snapshot = NewKpiSnapshot {
            kpi: KPI_TRUST_VARIANCE.into(),
            value: tv_value,
            confidence: tv_conf,
            numerator: None,
            denominator: None,
            sample_count: tv_sample as i64,
            window_start_ms: window_start,
            window_end_ms: now,
            computed_at_ms: now,
            sha: sha.clone(),
        };
        let tv_id = self
            .kpi_repo
            .insert_snapshot_with_lineage(&tv_snapshot, &tv_lineage)
            .await
            .map_err(|e| format!("trust-variance persist failed: {e}"))?;

        // --- fire the standing REC-4 canary on observed live traffic ------
        let evidence = format!(
            "KPI snapshots persisted: augmentation_ratio={ar_value:.3} (id={ar_id}, \
             agent_volume={agent_volume}, escalations={escalation_volume}), \
             trust_variance={tv_value:.3} (id={tv_id}, sample={tv_sample})"
        );
        if let Err(e) = self.harness.observe(CANARY_REC4_KPI, &evidence).await {
            warn!("[kpi] failed to record {CANARY_REC4_KPI} fire: {e}");
        } else {
            debug!("[kpi] {CANARY_REC4_KPI} fired: {evidence}");
        }

        // --- assemble the four-tile summary -------------------------------
        let ar_tile = KpiTile {
            kpi: KPI_AUGMENTATION_RATIO.to_string(),
            label: "Augmentation Ratio".to_string(),
            status: "computed",
            value: Some(ar_value),
            confidence: Some(ar_conf),
            unit: Some("ratio"),
            numerator: Some(agent_volume as f64),
            denominator: Some(escalation_volume as f64),
            sample_count: Some((agent_volume + escalation_volume) as i64),
            snapshot_id: Some(ar_id),
            window_days: Some(KPI_WINDOW_MS / (24 * 60 * 60 * 1000)),
            source: "agent-action volume (/wss/agent-events) ÷ ACSP escalation volume (enrichment_decisions)",
        };
        let tv_tile = KpiTile {
            kpi: KPI_TRUST_VARIANCE.to_string(),
            label: "Trust Variance".to_string(),
            status: "computed",
            value: Some(tv_value),
            confidence: Some(tv_conf),
            unit: Some("index"),
            numerator: None,
            denominator: None,
            sample_count: Some(tv_sample as i64),
            snapshot_id: Some(tv_id),
            window_days: Some(KPI_WINDOW_MS / (24 * 60 * 60 * 1000)),
            source: "Gini-Simpson dispersion of enrichment_decisions outcomes (30-day rolling)",
        };
        let mesh_tile = KpiTile::awaiting(
            KPI_MESH_VELOCITY,
            "Mesh Velocity",
            "REC-10 insight-loop timestamps (ontology_propose → broker decision → merged enrichment)",
        );
        let hitl_tile = KpiTile::awaiting(
            KPI_HITL_PRECISION,
            "HITL Precision",
            "broker decision outcomes surfaced by WP-4 case queue (HITL material-change flag)",
        );

        Ok(KpiSummary {
            tiles: vec![ar_tile, tv_tile, mesh_tile, hitl_tile],
            computed_at_ms: now,
            window_days: KPI_WINDOW_MS / (24 * 60 * 60 * 1000),
            sha,
        })
    }

    /// Lineage rows for a persisted snapshot (`GET /api/kpi/lineage/{id}`).
    pub async fn lineage_for(
        &self,
        snapshot_id: i64,
    ) -> Result<Vec<crate::adapters::sqlite_kpi_repository::KpiLineageRow>, String> {
        self.kpi_repo
            .lineage_for(snapshot_id)
            .await
            .map_err(|e| format!("lineage read failed: {e}"))
    }
}

/// Passive agent-action volume tap. Subscribes to the process-global
/// `/wss/agent-events` hub (the same seam the render actor uses) and records one
/// lightweight volume row per envelope. No change to the emit site or the wire —
/// the volume is read from an existing stream (ADR-130 D5). Never returns;
/// fail-open on a lagged/closed channel.
pub async fn run_agent_event_tap(kpi_repo: Arc<SqliteKpiRepository>) {
    let mut rx = crate::agent_events::hub::subscribe();
    log::info!("[kpi] agent-event volume tap subscribed to the agent-events hub");
    loop {
        match rx.recv().await {
            Ok(env) => {
                let observed_at_ms = chrono::Utc::now().timestamp_millis();
                if let Err(e) = kpi_repo
                    .record_agent_event(
                        env.id,
                        env.source_agent_id,
                        env.action_type,
                        observed_at_ms,
                    )
                    .await
                {
                    warn!("[kpi] failed to record agent-event volume: {e}");
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                // Volume is a count; a dropped frame under backpressure only
                // undercounts slightly. Resync on the next frame.
                debug!("[kpi] agent-event tap lagged {n} frames");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                warn!("[kpi] agent-event hub closed; volume tap stopping");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn augmentation_ratio_divides_volume_by_escalations() {
        // 42 agent actions, 12 escalations ⇒ 3.5.
        let (value, conf) = augmentation_ratio(42, 12);
        assert!((value - 3.5).abs() < 1e-9);
        assert!((conf - 1.0).abs() < 1e-9, "54 samples ≥ 30 ⇒ full confidence");
    }

    #[test]
    fn augmentation_ratio_zero_escalations_is_zero_confidence() {
        let (value, conf) = augmentation_ratio(100, 0);
        assert_eq!(value, 0.0);
        assert_eq!(conf, 0.0, "an undefined ratio carries no confidence");
    }

    #[test]
    fn augmentation_ratio_low_sample_scales_confidence() {
        let (value, conf) = augmentation_ratio(3, 3);
        assert!((value - 1.0).abs() < 1e-9);
        assert!((conf - 6.0 / 30.0).abs() < 1e-9, "6 samples ⇒ 0.2 confidence");
    }

    #[test]
    fn trust_variance_uniform_outcome_is_zero() {
        let outcomes = vec!["approve".to_string(); 10];
        let (value, _conf, sample) = trust_variance(&outcomes);
        assert_eq!(value, 0.0, "all one outcome ⇒ no dispersion");
        assert_eq!(sample, 10);
    }

    #[test]
    fn trust_variance_even_split_is_maximal() {
        let mut outcomes = vec!["approve".to_string(); 5];
        outcomes.extend(vec!["reject".to_string(); 5]);
        let (value, _conf, sample) = trust_variance(&outcomes);
        assert!((value - 1.0).abs() < 1e-9, "even 50/50 split ⇒ normalised 1.0");
        assert_eq!(sample, 10);
    }

    #[test]
    fn trust_variance_three_way_even_is_maximal() {
        let mut outcomes = vec!["approve".to_string(); 4];
        outcomes.extend(vec!["reject".to_string(); 4]);
        outcomes.extend(vec!["amend".to_string(); 4]);
        let (value, _conf, sample) = trust_variance(&outcomes);
        // Even spread across k=3 categories ⇒ normalised to 1.0.
        assert!((value - 1.0).abs() < 1e-9);
        assert_eq!(sample, 12);
    }

    #[test]
    fn trust_variance_skewed_is_between_zero_and_one() {
        // 9 approve, 1 reject ⇒ some but low dispersion.
        let mut outcomes = vec!["approve".to_string(); 9];
        outcomes.push("reject".to_string());
        let (value, _conf, _sample) = trust_variance(&outcomes);
        assert!(value > 0.0 && value < 0.5, "skewed split ⇒ low dispersion, got {value}");
    }

    #[test]
    fn trust_variance_empty_is_zero() {
        let (value, conf, sample) = trust_variance(&[]);
        assert_eq!(value, 0.0);
        assert_eq!(conf, 0.0);
        assert_eq!(sample, 0);
    }
}
