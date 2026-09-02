//! DecisionElevationActor — the write-half actor of ADR-050 (decision elevation).
//!
//! The mirror image of [`crate::actors::elevation_actor::ElevationActor`] for
//! `dl:DecisionRecord` instances. It receives a just-recorded SIGNIFICANT
//! decision from the governed write door (via the [`DecisionElevationSink`] seam,
//! implemented here by [`ActorElevationSink`]), drafts the decision as a corpus
//! page, opens a `DecisionElevation` broker case (ACSP `ActionRequest`, kind
//! 31402) on the forum governance panel, and — on a human `approve` (kind 31403)
//! — PRs the page into the corpus via [`GitHubPRService`] and tracks the PR to a
//! terminal git state (the GOV-2 merge poll). No auto-PR: the broker human gate
//! is mandatory — a decision page is only PR'd after an `approve` decision.
//!
//! Deliberately leaner than `ElevationActor`: decisions are ABox `prov:Activity`
//! individuals that add no TBox axioms, so there is NO EL++ consistency gate
//! (an approved decision page cannot make the classified graph inconsistent).
//! Boot is env-gated exactly like `ElevationActor` (dev/staging default ON,
//! production opt-in; needs `FORUM_RELAY_URL` + a panel secret to publish).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use log::{error, info, warn};
use serde_json::json;

use crate::services::acsp::events::{
    ActionDef, ActionStyle, FieldDef, FieldType, LayoutHint, PanelDefinition, PanelSchema,
};
use crate::services::acsp::{
    build_action_request, build_case_status_update, build_panel_definition, build_panel_state,
    AcspClient, ActionPriority, ActionRequest, CaseCategory, CaseDecision, CaseSpec, SubjectKind,
};
use crate::services::decision_elevation::{
    decision_slug, draft_decision_page, DecisionElevationSink, ElevatedDecision,
};
use crate::services::github_pr_service::{GitHubPRService, PrState};
use crate::types::ontology_tools::AgentContext;

/// NIP-33 panel id (the `d` tag) for the decision-elevation control surface.
const PANEL_ID: &str = "vc-decision-elevation";
/// Case-id namespace; the decision subscription filters on this prefix.
const CASE_PREFIX: &str = "vc-decelev-";
/// How many decision-elevation cases may be open at once (bounds broker volume).
const MAX_OPEN_CASES: usize = 16;
/// GOV-2 merge-poll cadence: how often opened elevation PRs are checked for a
/// terminal git state (merged → `decision_elevated`, closed → abandoned).
const PR_POLL_INTERVAL: Duration = Duration::from_secs(120);

/// The elevation trigger: a significant decision to route into the corpus.
#[derive(Message)]
#[rtype(result = "()")]
struct ElevateDecision(ElevatedDecision);

#[derive(Message)]
#[rtype(result = "()")]
struct Decision(CaseDecision);

/// Poll tracked elevation PRs for a terminal git state (GOV-2).
#[derive(Message)]
#[rtype(result = "()")]
struct PollPrs;

/// An opened decision-elevation PR being tracked to its terminal state (GOV-2).
#[derive(Debug, Clone)]
struct TrackedPr {
    pr_url: String,
    decision_urn: String,
}

#[derive(Debug, Clone)]
struct PendingCase {
    decision_urn: String,
    file_path: String,
    draft: String,
    summary: String,
}

pub struct DecisionElevationActor {
    acsp: Option<Arc<AcspClient>>,
    panel_secret: String,
    forum_relay_url: String,
    /// Open broker cases awaiting a human decision, keyed by case id.
    pending: HashMap<String, PendingCase>,
    /// GOV-2: elevation PRs opened on approve, tracked to their terminal git state.
    elevating: HashMap<String, TrackedPr>,
    /// Decision URNs already cased this session (idempotent trigger skip-list).
    seen: HashSet<String>,
    elevated_count: u32,
    rejected_count: u32,
    last_pr_url: Option<String>,
}

/// Pure production check (mirrors `elevation_actor::is_production_from`) so the
/// dev-default-ON / prod-opt-in gate is unit-testable without env mutation.
fn is_production_from(app_env: Option<String>, node_env: Option<String>) -> bool {
    let is_prod = |v: &Option<String>| {
        v.as_deref()
            .map(|s| s.eq_ignore_ascii_case("production"))
            .unwrap_or(false)
    };
    is_prod(&app_env) || is_prod(&node_env)
}

fn is_production_env() -> bool {
    is_production_from(
        std::env::var("APP_ENV").ok(),
        std::env::var("NODE_ENV").ok(),
    )
}

impl DecisionElevationActor {
    /// Construct if enabled + configured. `DECISION_ELEVATION_ENABLED` defaults
    /// ON in dev/staging and opt-in in production (an explicit value wins), and
    /// publishing additionally requires `FORUM_RELAY_URL` + a panel secret —
    /// `None` means the write-half is dormant for this profile/config (the
    /// governed decision write door then simply never sees a live sink).
    pub fn new() -> Option<Self> {
        let enabled = match std::env::var("DECISION_ELEVATION_ENABLED") {
            Ok(v) => v == "1" || v.eq_ignore_ascii_case("true"),
            Err(_) => !is_production_env(),
        };
        if !enabled {
            return None;
        }
        let forum_relay_url = std::env::var("FORUM_RELAY_URL").ok()?;
        let panel_secret = std::env::var("ACSP_PANEL_NOSTR_PRIVKEY")
            .or_else(|_| std::env::var("VISIONCLAW_NOSTR_PRIVKEY"))
            .ok()?;
        Some(Self {
            acsp: None,
            panel_secret,
            forum_relay_url,
            pending: HashMap::new(),
            elevating: HashMap::new(),
            seen: HashSet::new(),
            elevated_count: 0,
            rejected_count: 0,
            last_pr_url: None,
        })
    }

    fn panel_definition() -> PanelDefinition {
        PanelDefinition {
            title: "Decision Elevation".into(),
            description: "Significant decision records proposed for the public corpus. \
                          Approve to commit the decision page as a PR so the next sync \
                          re-derives it into the asserted graph (ADR-050)."
                .into(),
            version: "1.0.0".into(),
            schema: PanelSchema::ActionInbox,
            fields: vec![
                FieldDef {
                    name: "summary".into(),
                    field_type: FieldType::String,
                    label: "Decision".into(),
                },
                FieldDef {
                    name: "decision_urn".into(),
                    field_type: FieldType::String,
                    label: "Decision URN".into(),
                },
                FieldDef {
                    name: "causal_edges".into(),
                    field_type: FieldType::Json,
                    label: "Causal edges".into(),
                },
                FieldDef {
                    name: "file_path".into(),
                    field_type: FieldType::String,
                    label: "Corpus path".into(),
                },
            ],
            actions: vec![
                ActionDef {
                    id: "approve".into(),
                    label: "Publish".into(),
                    style: ActionStyle::Primary,
                },
                ActionDef {
                    id: "reject".into(),
                    label: "Keep runtime-only".into(),
                    style: ActionStyle::Secondary,
                },
            ],
            layout: LayoutHint::InboxTable,
            capabilities: vec![],
            refresh_secs: 60,
        }
    }

    fn state_snapshot(&self) -> serde_json::Value {
        json!({
            "open_cases": self.pending.len(),
            "elevated": self.elevated_count,
            "rejected": self.rejected_count,
            "awaiting_merge": self.elevating.len(),
            "last_pr_url": self.last_pr_url,
        })
    }

    fn publish_state(&self, ctx: &mut Context<Self>) {
        let Some(acsp) = self.acsp.clone() else {
            return;
        };
        let state = self.state_snapshot();
        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                if let Err(e) = acsp.publish(&build_panel_state(PANEL_ID, &state)).await {
                    warn!("[DecisionElevation] panel state publish failed: {e}");
                }
            })
            .map(|_, _, _| ()),
        );
    }
}

impl Actor for DecisionElevationActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!(
            "[DecisionElevation] actor starting (panel '{PANEL_ID}', case prefix '{CASE_PREFIX}')"
        );
        let secret = self.panel_secret.clone();
        let relay = self.forum_relay_url.clone();
        let addr = ctx.address();

        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                match AcspClient::connect(&secret, &relay).await {
                    Ok(client) => {
                        let client = Arc::new(client);
                        let def = build_panel_definition(PANEL_ID, &Self::panel_definition());
                        if let Err(e) = client.publish(&def).await {
                            warn!("[DecisionElevation] panel definition publish failed: {e}");
                        }
                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CaseDecision>();
                        let sub_client = client.clone();
                        tokio::spawn(async move {
                            sub_client
                                .run_decision_subscription(CASE_PREFIX.into(), tx)
                                .await;
                        });
                        let fwd_addr = addr.clone();
                        tokio::spawn(async move {
                            while let Some(d) = rx.recv().await {
                                fwd_addr.do_send(Decision(d));
                            }
                        });
                        Some(client)
                    }
                    Err(e) => {
                        error!("[DecisionElevation] ACSP connect failed: {e}; actor idle");
                        None
                    }
                }
            })
            .map(|client, act, _ctx| {
                act.acsp = client;
            }),
        );

        // GOV-2: poll opened elevation PRs for a terminal git state.
        if GitHubPRService::has_github_token() {
            info!("[DecisionElevation] GOV-2 merge poll armed (every {}s) — merged PRs fire decision_elevated", PR_POLL_INTERVAL.as_secs());
        } else {
            warn!("[DecisionElevation] GOV-2 DEGRADED: no GitHub token (PRIVATE_REPO_GITHUB_PAT) — elevation PRs cannot be opened and merge polling cannot resolve; decision_elevated will never fire until a token is configured");
        }
        ctx.run_interval(PR_POLL_INTERVAL, |_act, ctx| {
            ctx.address().do_send(PollPrs);
        });
    }
}

impl Handler<ElevateDecision> for DecisionElevationActor {
    type Result = ();

    fn handle(&mut self, ElevateDecision(dec): ElevateDecision, ctx: &mut Self::Context) {
        // Idempotent + bounded: skip a decision already cased this session, and
        // never exceed the open-case budget (fail-open — dropping is safe: the
        // decision is durable at runtime and the read-half re-derives elevated ones).
        if self.seen.contains(&dec.decision_urn) {
            return;
        }
        if self.pending.len() >= MAX_OPEN_CASES {
            warn!(
                "[DecisionElevation] open-case budget ({MAX_OPEN_CASES}) reached; decision {} stays runtime-only until a case frees",
                dec.decision_urn
            );
            return;
        }
        let Some(acsp) = self.acsp.clone() else {
            // Not yet connected (or connect failed) — fail-open, drop the trigger.
            return;
        };

        let (file_path, draft) = draft_decision_page(&dec);
        let slug = decision_slug(&dec.decision_urn, &dec.input.summary);
        let case_id = format!("{CASE_PREFIX}{slug}");
        let summary = if dec.input.summary.trim().is_empty() {
            dec.decision_urn.clone()
        } else {
            dec.input.summary.clone()
        };

        let causal: Vec<String> = dec
            .input
            .caused
            .iter()
            .chain(dec.input.precedent_for.iter())
            .chain(dec.input.influenced.iter())
            .cloned()
            .collect();
        let priority = if dec.acsp_approved {
            ActionPriority::High
        } else {
            ActionPriority::Medium
        };
        let reasoning = format!(
            "Significant decision (caused {} / precedent {} / influenced {}{}) proposed for the \
             public corpus so the force_full assert-graph rebuild re-derives it (ADR-050).",
            dec.input.caused.len(),
            dec.input.precedent_for.len(),
            dec.input.influenced.len(),
            if dec.input.proposal_urn.is_some() {
                "; governed a graph mutation"
            } else {
                ""
            },
        );
        let spec = CaseSpec {
            case_id: case_id.clone(),
            title: format!("Elevate decision: {summary}"),
            priority,
            category: CaseCategory::DecisionElevation,
            subject_kind: SubjectKind::AutomationProposal,
            subject_id: dec.decision_urn.clone(),
            request: ActionRequest {
                fields: json!({
                    "summary": summary,
                    "decision_urn": dec.decision_urn,
                    "causal_edges": causal,
                    "file_path": file_path.clone(),
                    "acsp_approved": dec.acsp_approved,
                }),
                reasoning: Some(reasoning),
                context_url: None,
            },
        };

        let pending = PendingCase {
            decision_urn: dec.decision_urn.clone(),
            file_path,
            draft,
            summary,
        };

        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                acsp.publish(&build_action_request(&spec)).await.map(|_| ())
            })
            .map(move |result, act, ctx| match result {
                Ok(()) => {
                    info!(
                        "[DecisionElevation] opened case {case_id} for decision {}",
                        pending.decision_urn
                    );
                    act.seen.insert(pending.decision_urn.clone());
                    act.pending.insert(case_id, pending);
                    act.publish_state(ctx);
                }
                Err(e) => warn!("[DecisionElevation] case publish failed: {e}"),
            }),
        );
    }
}

impl Handler<Decision> for DecisionElevationActor {
    type Result = ();

    fn handle(&mut self, Decision(d): Decision, ctx: &mut Self::Context) {
        let Some(case) = self.pending.remove(&d.case_id) else {
            return; // replayed / foreign decision
        };
        info!(
            "[DecisionElevation] case {} decided '{}' by {} — {}",
            d.case_id, d.action, d.responder_pubkey, d.reasoning
        );

        match d.action.as_str() {
            "approve" => {
                // Broker human gate satisfied → PR the decision page into the corpus.
                let case_id = d.case_id.clone();
                let decision_urn = case.decision_urn.clone();
                let summary = case.summary.clone();
                let file_path = case.file_path.clone();
                let draft = case.draft.clone();
                let agent_id = format!("decision-{}", decision_slug(&decision_urn, &summary));
                ctx.spawn(
                    actix::fut::wrap_future::<_, Self>(async move {
                        let pr = GitHubPRService::new();
                        let agent_ctx = AgentContext {
                            agent_id,
                            agent_type: "decision-elevation".into(),
                            task_description: format!(
                                "ADR-050 broker-approved elevation of decision {decision_urn}"
                            ),
                            session_id: None,
                            confidence: 0.5,
                            user_id: "acsp-governance".into(),
                        };
                        pr.create_ontology_pr(
                            &file_path,
                            &draft,
                            &format!("feat(decision): elevate decision '{summary}' to corpus"),
                            &format!(
                                "Corpus page for decision record **{summary}**\n\n\
                                 URN: `{decision_urn}`\n\n\
                                 Approved via the forum governance panel (ADR-050 decision \
                                 elevation). The next `force_full` sync re-derives this decision \
                                 into `urn:ngm:graph:ontology:assert`; signed attribution stays in \
                                 the provenance graph.\n\n🤖 Generated by Claude Code"
                            ),
                            &agent_ctx,
                        )
                        .await
                        .map(|url| (case_id, decision_urn, url))
                    })
                    .map(|result, act, ctx| {
                        match result {
                            Ok((case_id, decision_urn, url)) => {
                                act.elevated_count += 1;
                                act.last_pr_url = Some(url.clone());
                                act.elevating.insert(
                                    case_id.clone(),
                                    TrackedPr {
                                        pr_url: url.clone(),
                                        decision_urn,
                                    },
                                );
                                info!("[DecisionElevation] PR created: {url} (tracking case {case_id} for merge → decision_elevated)");
                            }
                            Err(e) => error!("[DecisionElevation] PR creation failed: {e}"),
                        }
                        act.publish_state(ctx);
                    }),
                );
            }
            _ => {
                // reject / amend / delegate — the decision stays runtime-only.
                self.rejected_count += 1;
                self.publish_state(ctx);
            }
        }
    }
}

/// GOV-2: map a terminal PR git state to `(31404 status)`. `Open` ⇒ keep polling.
fn terminal_for_pr_state(state: PrState) -> Option<&'static str> {
    match state {
        PrState::Merged => Some("decision_elevated"),
        PrState::ClosedUnmerged => Some("decision_abandoned"),
        PrState::Open => None,
    }
}

impl Handler<PollPrs> for DecisionElevationActor {
    type Result = ();

    fn handle(&mut self, _msg: PollPrs, ctx: &mut Self::Context) {
        if self.elevating.is_empty() {
            return;
        }
        if !GitHubPRService::has_github_token() {
            warn!(
                "[DecisionElevation] GOV-2 merge poll DEGRADED: {} PR(s) tracked but no GitHub token; decision_elevated cannot fire until PRIVATE_REPO_GITHUB_PAT is configured",
                self.elevating.len()
            );
            return;
        }
        let Some(acsp) = self.acsp.clone() else {
            return;
        };
        let tracked: Vec<(String, TrackedPr)> = self
            .elevating
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let pr = GitHubPRService::new();

        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                let mut resolved: Vec<String> = Vec::new();
                for (case_id, tracked) in tracked {
                    match pr.pr_state(&tracked.pr_url).await {
                        Ok(state) => {
                            let Some(status) = terminal_for_pr_state(state) else {
                                continue; // still open — keep tracking
                            };
                            if let Err(e) = acsp
                                .publish(&build_case_status_update(
                                    PANEL_ID,
                                    &case_id,
                                    status,
                                    &tracked.pr_url,
                                ))
                                .await
                            {
                                warn!("[DecisionElevation] GOV-2 31404 publish failed for {case_id}: {e}");
                            }
                            info!(
                                "[DecisionElevation] case {case_id} terminal: {status} for decision {} ({})",
                                tracked.decision_urn, tracked.pr_url
                            );
                            resolved.push(case_id);
                        }
                        Err(e) => {
                            warn!("[DecisionElevation] GOV-2 PR state poll failed for {case_id}: {e}");
                        }
                    }
                }
                resolved
            })
            .map(|resolved, act, _ctx| {
                for case_id in resolved {
                    act.elevating.remove(&case_id);
                }
            }),
        );
    }
}

/// Adapter making a live [`DecisionElevationActor`] address satisfy the
/// [`DecisionElevationSink`] the governed write door calls. `elevate` is
/// fire-and-forget: `do_send` never blocks, and a dead mailbox is reported as
/// `Err` (the caller logs and ignores — ADR-050 fail-open).
pub struct ActorElevationSink {
    addr: Addr<DecisionElevationActor>,
}

impl ActorElevationSink {
    pub fn new(addr: Addr<DecisionElevationActor>) -> Self {
        Self { addr }
    }
}

impl DecisionElevationSink for ActorElevationSink {
    fn elevate(&self, decision: ElevatedDecision) -> Result<(), String> {
        self.addr
            .try_send(ElevateDecision(decision))
            .map_err(|e| format!("decision-elevation mailbox send failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_gate_defaults_dev_on_prod_off() {
        assert!(!is_production_from(None, None));
        assert!(is_production_from(Some("production".into()), None));
        assert!(is_production_from(None, Some("Production".into())));
        assert!(!is_production_from(Some("staging".into()), None));
    }

    #[test]
    fn terminal_pr_state_maps_merge_and_abandon() {
        assert_eq!(
            terminal_for_pr_state(PrState::Merged),
            Some("decision_elevated")
        );
        assert_eq!(
            terminal_for_pr_state(PrState::ClosedUnmerged),
            Some("decision_abandoned")
        );
        assert_eq!(terminal_for_pr_state(PrState::Open), None);
    }
}
