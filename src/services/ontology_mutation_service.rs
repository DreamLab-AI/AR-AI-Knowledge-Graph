//! OntologyMutationService — Agent write path for the living ontology corpus.
//!
//! Agents propose new notes or amendments to existing notes. Each proposal runs
//! through the W-E governed transaction spine (ADR-049 / DDD-020):
//!
//!   0. Signature-envelope precondition (fail-closed; verified BEFORE any mutation)
//!   1. Idempotency reservation — a single proposal id + idempotency key span
//!      every stage; replay of a key with an identical payload returns the prior
//!      receipt, replay with a different payload is rejected.
//!   2. Write-ahead intent recorded `Pending` before the mutation.
//!   3. Conflict-integrity gate (W-A) — the four `conflicts.py` detectors.
//!   4. REAL Whelk EL++ consistency over corpus ∪ proposed axioms
//!      (`check_axiom_set`), replacing the former no-op gate.
//!   5. Governance / projection — currently a GitHub PR for human review; the
//!      native single-transaction asserted+provenance write lands in Phase 2
//!      once the provenance-writer (T3) quad builders and the injected Oxigraph
//!      store seam are available. The intent is marked `Committed` on success.
//!   6. A deterministic [`ProposalReceipt`] is minted and returned.
//!
//! Notes are per-user — each user's agents write to their own namespace.

use crate::adapters::whelk_inference_engine::WhelkInferenceEngine;
use visionclaw_domain::ports::ontology_repository::{OwlAxiom, AxiomType, OntologyRepository, OwlClass};
use crate::services::file_service::MARKDOWN_DIR;
use crate::services::github_pr_service::GitHubPRService;
use crate::services::ontology_conflict_gate::{evaluate as evaluate_conflicts, ProposedCandidate};
use crate::services::proposal_spine::{
    build_receipt, payload_hash, IdempotencyDecision, IdempotencyStore, IntentLog, IntentState,
    ReceiptInputs, WriteAheadIntent,
};
use crate::types::ontology_tools::*;
use chrono::Utc;
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error-string sentinel prefix: a blocking conflict-integrity report. The
/// handler maps this to HTTP 409 and returns the serialised [`ConflictReport`].
pub const CONFLICT_BLOCKED_PREFIX: &str = "CONFLICT_BLOCKED:";
/// Error-string sentinel prefix: an idempotency-key replay with a divergent
/// payload. The handler maps this to HTTP 409.
pub const IDEMPOTENCY_CONFLICT_PREFIX: &str = "IDEMPOTENCY_CONFLICT:";
/// Error-string sentinel prefix: the signature-envelope precondition rejected
/// the proposal before any mutation (fail-closed). Handler maps to HTTP 403.
pub const ENVELOPE_REJECTED_PREFIX: &str = "ENVELOPE_REJECTED:";

/// Relationship keys that feed the conflict gate's `contrasts_with` axis.
const CONTRASTS_WITH_KEYS: [&str; 2] = ["contrasts-with", "contrasts_with"];

pub struct OntologyMutationService {
    ontology_repo: Arc<dyn OntologyRepository>,
    /// Retained for the future native-projection reload path; the live gate uses
    /// the static, re-entrant `WhelkInferenceEngine::check_axiom_set`.
    #[allow(dead_code)]
    whelk: Arc<RwLock<WhelkInferenceEngine>>,
    github_pr: Arc<GitHubPRService>,
    /// W-E: idempotency persistence (replay-safe proposal receipts).
    idempotency: Arc<dyn IdempotencyStore>,
    /// W-E: write-ahead intent log for deterministic recovery.
    intents: Arc<dyn IntentLog>,
}

impl OntologyMutationService {
    pub fn new(
        ontology_repo: Arc<dyn OntologyRepository>,
        whelk: Arc<RwLock<WhelkInferenceEngine>>,
        github_pr: Arc<GitHubPRService>,
        idempotency: Arc<dyn IdempotencyStore>,
        intents: Arc<dyn IntentLog>,
    ) -> Self {
        Self {
            ontology_repo,
            whelk,
            github_pr,
            idempotency,
            intents,
        }
    }

    /// Propose creating a new note in the ontology corpus.
    pub async fn propose_create(
        &self,
        proposal: NoteProposal,
        agent_ctx: AgentContext,
        idempotency_key: Option<String>,
    ) -> Result<ProposalResult, String> {
        info!(
            "Ontology propose_create: term='{}', agent={} (user={})",
            proposal.preferred_term, agent_ctx.agent_id, agent_ctx.user_id
        );

        // W-E stage: single proposal id spanning every stage (minted at START).
        let proposal_id = uuid::Uuid::new_v4().to_string();

        // W-E stage 0: signature-envelope precondition (fail-closed, pre-mutation).
        Self::verify_envelope_precondition(&agent_ctx)?;

        // W-E stage 1: idempotency. Canonical payload hash over the create body.
        let payload = serde_json::json!({ "action": "create", "proposal": proposal });
        let phash = payload_hash(&payload);
        let idem_key = idempotency_key.unwrap_or_else(|| format!("auto:{}", phash));
        match self.idempotency.reserve(&idem_key, &phash) {
            IdempotencyDecision::Replay(receipt) => {
                info!("propose_create: idempotent replay for key={}", idem_key);
                return Ok(Self::replay_result("create", &proposal.owl_class, receipt));
            }
            IdempotencyDecision::Conflict => {
                return Err(format!(
                    "{}idempotency key '{}' already used with a different payload",
                    IDEMPOTENCY_CONFLICT_PREFIX, idem_key
                ));
            }
            IdempotencyDecision::Fresh => {}
        }

        // W-E stage 2: write-ahead intent (Pending) BEFORE any mutation.
        self.intents
            .record(WriteAheadIntent::pending(&proposal_id, &idem_key, &phash));

        // Build the axiom set used by BOTH the conflict gate and Whelk.
        let proposed_axioms: Vec<OwlAxiom> = proposal
            .is_subclass_of
            .iter()
            .map(|parent| OwlAxiom {
                id: None,
                axiom_type: AxiomType::SubClassOf,
                subject: proposal.owl_class.clone(),
                object: parent.clone(),
                annotations: std::collections::HashMap::new(),
            })
            .collect();

        // Load the corpus once — reused by the conflict gate and the Whelk gate.
        let corpus = self.ontology_repo.list_owl_classes().await.unwrap_or_default();

        // W-E stage 3: conflict-integrity gate (W-A / DDD-020 I07).
        let candidate = ProposedCandidate {
            iri: proposal.owl_class.clone(),
            label: proposal.preferred_term.clone(),
            entity_type: "Class".to_string(),
            subclass_of: proposal.is_subclass_of.clone(),
            contrasts_with: extract_contrasts(&proposal.relationships),
        };
        let conflict_report = evaluate_conflicts(&corpus, &candidate);
        if !conflict_report.ok() {
            warn!(
                "propose_create rejected — {} blocking conflict(s), {} pre-existing advisory",
                conflict_report.blocking.len(),
                conflict_report.pre_existing.len()
            );
            self.intents.mark(&proposal_id, IntentState::Failed);
            return Err(format!(
                "{}{}",
                CONFLICT_BLOCKED_PREFIX,
                serde_json::to_string(&conflict_report).unwrap_or_default()
            ));
        }

        // W-E stage 4: REAL Whelk EL++ consistency (corpus ∪ proposed axioms).
        let consistency = self.check_consistency(&corpus, &proposed_axioms);
        if !consistency.consistent {
            warn!(
                "propose_create rejected — inconsistent: {:?}",
                consistency.explanation
            );
            self.intents.mark(&proposal_id, IntentState::Failed);
            let gates = GateSummary::pending()
                .with_conflict(GateOutcome::Pass)
                .with_whelk(false)
                .with_acsp(GateOutcome::Pending);
            return Ok(ProposalResult {
                proposal_id,
                action: "create".to_string(),
                target_iri: proposal.owl_class.clone(),
                consistency,
                quality_score: 0.0,
                markdown_preview: String::new(),
                pr_url: None,
                status: ProposalStatus::Rejected,
                receipt: None,
                gates: Some(gates),
            });
        }

        // ----- gates passed; proceed to projection/governance -----
        let term_id = match self.generate_term_id(&proposal.domain).await {
            Ok(t) => t,
            Err(e) => {
                self.intents.mark(&proposal_id, IntentState::Failed);
                return Err(e);
            }
        };
        let markdown = self.generate_logseq_markdown(&proposal, &term_id, &agent_ctx.user_id);
        let quality_score = self.compute_quality_score(&proposal);

        let file_path = format!(
            "{}/{}/{}.md",
            MARKDOWN_DIR,
            proposal.domain,
            term_id.to_lowercase().replace('-', "_")
        );

        // W-E stage 5: governance projection (GitHub PR for human review).
        let pr_url = match self
            .github_pr
            .create_ontology_pr(
                &file_path,
                &markdown,
                &format!(
                    "[ontology] {}: Add {}",
                    agent_ctx.agent_type, proposal.preferred_term
                ),
                &self.build_pr_body(&proposal, &agent_ctx, &consistency, quality_score),
                &agent_ctx,
            )
            .await
        {
            Ok(url) => Some(url),
            Err(e) => {
                error!("Failed to create GitHub PR: {}", e);
                None
            }
        };

        // W-E stage 6: mint the deterministic receipt, commit idempotency + intent.
        let assert_triples = create_assert_triples(&proposal);
        let provenance_quads =
            provenance_seed(&proposal_id, &proposal.owl_class, &agent_ctx.agent_id);
        let receipt = build_receipt(&ReceiptInputs {
            proposal_id: &proposal_id,
            idempotency_key: &idem_key,
            assert_triples: &assert_triples,
            provenance_quads: &provenance_quads,
            envelope: None,
        });
        self.idempotency.commit(&idem_key, &phash, receipt.clone());
        self.intents.mark(&proposal_id, IntentState::Committed);

        let status = if pr_url.is_some() {
            ProposalStatus::PRCreated
        } else {
            ProposalStatus::Staged
        };
        let gates = GateSummary::pending()
            .with_conflict(GateOutcome::Pass)
            .with_whelk(true)
            .with_acsp(GateOutcome::Pending);

        info!(
            "propose_create {} committed: iri={}, status={:?}",
            proposal_id, proposal.owl_class, status
        );

        Ok(ProposalResult {
            proposal_id,
            action: "create".to_string(),
            target_iri: proposal.owl_class,
            consistency,
            quality_score,
            markdown_preview: markdown.chars().take(500).collect(),
            pr_url,
            status,
            receipt: Some(receipt),
            gates: Some(gates),
        })
    }

    /// Propose amending an existing note in the ontology corpus.
    pub async fn propose_amend(
        &self,
        target_iri: &str,
        amendment: NoteAmendment,
        agent_ctx: AgentContext,
        idempotency_key: Option<String>,
    ) -> Result<ProposalResult, String> {
        info!(
            "Ontology propose_amend: iri='{}', agent={} (user={})",
            target_iri, agent_ctx.agent_id, agent_ctx.user_id
        );

        let proposal_id = uuid::Uuid::new_v4().to_string();

        // Stage 0: envelope precondition.
        Self::verify_envelope_precondition(&agent_ctx)?;

        // Stage 1: idempotency.
        let payload = serde_json::json!({
            "action": "amend",
            "targetIri": target_iri,
            "amendment": amendment,
        });
        let phash = payload_hash(&payload);
        let idem_key = idempotency_key.unwrap_or_else(|| format!("auto:{}", phash));
        match self.idempotency.reserve(&idem_key, &phash) {
            IdempotencyDecision::Replay(receipt) => {
                info!("propose_amend: idempotent replay for key={}", idem_key);
                return Ok(Self::replay_result("amend", target_iri, receipt));
            }
            IdempotencyDecision::Conflict => {
                return Err(format!(
                    "{}idempotency key '{}' already used with a different payload",
                    IDEMPOTENCY_CONFLICT_PREFIX, idem_key
                ));
            }
            IdempotencyDecision::Fresh => {}
        }

        // Stage 2: write-ahead intent.
        self.intents
            .record(WriteAheadIntent::pending(&proposal_id, &idem_key, &phash));

        // Fetch existing class.
        let existing = match self.ontology_repo.get_owl_class(target_iri).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                self.intents.mark(&proposal_id, IntentState::Failed);
                return Err(format!("Class not found: {}", target_iri));
            }
            Err(e) => {
                self.intents.mark(&proposal_id, IntentState::Failed);
                return Err(format!("Failed to get class: {}", e));
            }
        };

        let existing_markdown = existing.markdown_content.clone().unwrap_or_default();
        let mut new_markdown = existing_markdown.clone();

        if let Some(ref new_def) = amendment.update_definition {
            if let Some(start) = new_markdown.find("definition::") {
                if let Some(end) = new_markdown[start..].find('\n') {
                    new_markdown
                        .replace_range(start..start + end, &format!("definition:: {}", new_def));
                }
            }
        }

        for (rel_type, targets) in &amendment.add_relationships {
            for target in targets {
                let line = format!("    - {}:: [[{}]]", rel_type, target);
                new_markdown.push_str(&format!("\n{}", line));
            }
        }

        // New subclass axioms proposed by the amendment.
        let mut proposed_axioms = Vec::new();
        let mut new_parents: Vec<String> = Vec::new();
        for (rel_type, targets) in &amendment.add_relationships {
            if rel_type == "is-subclass-of" {
                for target in targets {
                    new_parents.push(target.clone());
                    proposed_axioms.push(OwlAxiom {
                        id: None,
                        axiom_type: AxiomType::SubClassOf,
                        subject: target_iri.to_string(),
                        object: target.clone(),
                        annotations: std::collections::HashMap::new(),
                    });
                }
            }
        }

        let corpus = self.ontology_repo.list_owl_classes().await.unwrap_or_default();

        // Stage 3: conflict gate over the amended candidate.
        let mut subclass_of = existing.parent_classes.clone();
        subclass_of.extend(new_parents.iter().cloned());
        let candidate = ProposedCandidate {
            iri: target_iri.to_string(),
            label: existing.label.clone().unwrap_or_else(|| {
                existing.preferred_term.clone().unwrap_or_else(|| target_iri.to_string())
            }),
            entity_type: existing.class_type.clone().unwrap_or_else(|| "Class".to_string()),
            subclass_of,
            contrasts_with: extract_contrasts(&amendment.add_relationships),
        };
        let conflict_report = evaluate_conflicts(&corpus, &candidate);
        if !conflict_report.ok() {
            warn!(
                "propose_amend rejected — {} blocking conflict(s), {} pre-existing advisory",
                conflict_report.blocking.len(),
                conflict_report.pre_existing.len()
            );
            self.intents.mark(&proposal_id, IntentState::Failed);
            return Err(format!(
                "{}{}",
                CONFLICT_BLOCKED_PREFIX,
                serde_json::to_string(&conflict_report).unwrap_or_default()
            ));
        }

        // Stage 4: Whelk consistency (skip only when no new axioms are proposed).
        let consistency = if proposed_axioms.is_empty() {
            ConsistencyReport {
                consistent: true,
                new_subsumptions: 0,
                explanation: None,
            }
        } else {
            self.check_consistency(&corpus, &proposed_axioms)
        };

        if !consistency.consistent {
            self.intents.mark(&proposal_id, IntentState::Failed);
            let gates = GateSummary::pending()
                .with_conflict(GateOutcome::Pass)
                .with_whelk(false)
                .with_acsp(GateOutcome::Pending);
            return Ok(ProposalResult {
                proposal_id,
                action: "amend".to_string(),
                target_iri: target_iri.to_string(),
                consistency,
                quality_score: 0.0,
                markdown_preview: new_markdown.chars().take(500).collect(),
                pr_url: None,
                status: ProposalStatus::Rejected,
                receipt: None,
                gates: Some(gates),
            });
        }

        // Stage 5: governance projection.
        let file_path = existing.source_file.clone().unwrap_or_else(|| {
            let domain = existing.source_domain.as_deref().unwrap_or("general");
            let term_id = existing.term_id.as_deref().unwrap_or("unknown");
            format!(
                "{}/{}/{}.md",
                MARKDOWN_DIR,
                domain,
                term_id.to_lowercase().replace('-', "_")
            )
        });

        let pr_url = match self
            .github_pr
            .create_ontology_pr(
                &file_path,
                &new_markdown,
                &format!(
                    "[ontology] {}: Amend {}",
                    agent_ctx.agent_type,
                    existing.preferred_term.as_deref().unwrap_or(target_iri)
                ),
                &self.build_amend_pr_body(target_iri, &amendment, &agent_ctx, &consistency),
                &agent_ctx,
            )
            .await
        {
            Ok(url) => Some(url),
            Err(e) => {
                error!("Failed to create GitHub PR for amendment: {}", e);
                None
            }
        };

        // Stage 6: receipt + commit.
        let assert_triples = amend_assert_triples(target_iri, &new_parents);
        let provenance_quads = provenance_seed(&proposal_id, target_iri, &agent_ctx.agent_id);
        let receipt = build_receipt(&ReceiptInputs {
            proposal_id: &proposal_id,
            idempotency_key: &idem_key,
            assert_triples: &assert_triples,
            provenance_quads: &provenance_quads,
            envelope: None,
        });
        self.idempotency.commit(&idem_key, &phash, receipt.clone());
        self.intents.mark(&proposal_id, IntentState::Committed);

        let status = if pr_url.is_some() {
            ProposalStatus::PRCreated
        } else {
            ProposalStatus::Staged
        };
        let gates = GateSummary::pending()
            .with_conflict(GateOutcome::Pass)
            .with_whelk(true)
            .with_acsp(GateOutcome::Pending);

        Ok(ProposalResult {
            proposal_id,
            action: "amend".to_string(),
            target_iri: target_iri.to_string(),
            consistency,
            quality_score: amendment.update_quality_score.unwrap_or(0.5),
            markdown_preview: new_markdown.chars().take(500).collect(),
            pr_url,
            status,
            receipt: Some(receipt),
            gates: Some(gates),
        })
    }

    /// ADR-049 Security: the signature-envelope precondition runs BEFORE any
    /// mutation and is fail-closed. Native BIP-340 envelope verification is not
    /// yet landed; when `ONTOLOGY_REQUIRE_SIGNED_ENVELOPE` is set we reject by
    /// default (never silently pass an unverifiable envelope). Default-off keeps
    /// the current authenticated-route behaviour until the verifier lands.
    fn verify_envelope_precondition(agent_ctx: &AgentContext) -> Result<(), String> {
        let required = std::env::var("ONTOLOGY_REQUIRE_SIGNED_ENVELOPE")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"))
            .unwrap_or(false);
        if required {
            // No native envelope verifier is wired yet → fail closed.
            return Err(format!(
                "{}signed envelope required but no verifier is available for agent {}",
                ENVELOPE_REJECTED_PREFIX, agent_ctx.agent_id
            ));
        }
        Ok(())
    }

    /// Reconstruct a minimal replay result from a stored receipt (no re-mutation).
    fn replay_result(action: &str, target_iri: &str, receipt: ProposalReceipt) -> ProposalResult {
        ProposalResult {
            proposal_id: receipt.proposal_id.clone(),
            action: action.to_string(),
            target_iri: target_iri.to_string(),
            consistency: ConsistencyReport {
                consistent: true,
                new_subsumptions: 0,
                explanation: None,
            },
            quality_score: 0.0,
            markdown_preview: String::new(),
            pr_url: None,
            status: ProposalStatus::PRCreated,
            receipt: Some(receipt),
            gates: Some(
                GateSummary::pending()
                    .with_conflict(GateOutcome::Pass)
                    .with_whelk(true)
                    .with_acsp(GateOutcome::Pending),
            ),
        }
    }

    /// Generate the next term-id for a domain (e.g., AI-0851)
    async fn generate_term_id(&self, domain: &str) -> Result<String, String> {
        let prefix = match domain {
            "ai" => "AI",
            "bc" => "BC",
            "rb" => "RB",
            "mv" => "MV",
            "tc" => "TC",
            "dt" => "DT",
            _ => "GEN",
        };

        let classes = self.ontology_repo.list_owl_classes().await.unwrap_or_default();
        let max_seq = classes
            .iter()
            .filter_map(|c| {
                c.term_id.as_ref().and_then(|tid| {
                    if tid.starts_with(prefix) {
                        tid.split('-').last()?.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
            })
            .max()
            .unwrap_or(0);

        Ok(format!("{}-{:04}", prefix, max_seq + 1))
    }

    /// Generate valid Logseq markdown with OntologyBlock headers.
    fn generate_logseq_markdown(
        &self,
        proposal: &NoteProposal,
        term_id: &str,
        user_id: &str,
    ) -> String {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        let parents: Vec<String> = proposal
            .is_subclass_of
            .iter()
            .map(|p| format!("[[{}]]", p))
            .collect();
        let parents_str = parents.join(", ");

        let alt_terms_str = if proposal.alt_terms.is_empty() {
            String::new()
        } else {
            let terms: Vec<String> = proposal.alt_terms.iter().map(|t| format!("[[{}]]", t)).collect();
            format!("    - alt-terms:: {}\n", terms.join(", "))
        };

        let mut rels_section = String::new();
        for (rel_type, targets) in &proposal.relationships {
            for target in targets {
                rels_section.push_str(&format!("    - {}:: [[{}]]\n", rel_type, target));
            }
        }

        format!(
            r#"- {preferred_term}
  - ### OntologyBlock
    - ontology:: true
    - term-id:: {term_id}
    - preferred-term:: {preferred_term}
    - source-domain:: {domain}
    - status:: agent-proposed
    - public-access:: true
    - last-updated:: {today}
    - definition:: {definition}
    - owl:class:: {owl_class}
    - owl:physicality:: {physicality}
    - owl:role:: {role}
    - is-subclass-of:: {parents}
    - quality-score:: 0.6
    - authority-score:: 0.5
    - maturity:: draft
    - contributed-by:: {user_id}
{alt_terms}{relationships}"#,
            preferred_term = proposal.preferred_term,
            term_id = term_id,
            domain = proposal.domain,
            today = today,
            definition = proposal.definition,
            owl_class = proposal.owl_class,
            physicality = proposal.physicality,
            role = proposal.role,
            parents = parents_str,
            user_id = user_id,
            alt_terms = alt_terms_str,
            relationships = rels_section,
        )
    }

    /// REAL Whelk EL++ consistency check: builds the union of the corpus classes
    /// and the proposed axioms and runs the static, re-entrant `check_axiom_set`
    /// reasoner. This REPLACES the former no-op that ignored `proposed_axioms`
    /// and only inspected the already-loaded engine. A class whelk subsumes under
    /// `owl:Nothing` marks the proposal inconsistent.
    fn check_consistency(
        &self,
        corpus: &[OwlClass],
        proposed_axioms: &[OwlAxiom],
    ) -> ConsistencyReport {
        let outcome = WhelkInferenceEngine::check_axiom_set(corpus, proposed_axioms);
        ConsistencyReport {
            consistent: outcome.consistent,
            new_subsumptions: proposed_axioms.len(),
            explanation: if outcome.consistent {
                None
            } else {
                Some(outcome.explanation())
            },
        }
    }

    /// Compute quality score for a proposal based on completeness.
    fn compute_quality_score(&self, proposal: &NoteProposal) -> f32 {
        let mut score = 0.0f32;
        let mut fields = 0.0f32;

        if !proposal.preferred_term.is_empty() { score += 1.0; }
        fields += 1.0;
        if !proposal.definition.is_empty() { score += 1.0; }
        fields += 1.0;
        if !proposal.owl_class.is_empty() { score += 1.0; }
        fields += 1.0;
        if !proposal.is_subclass_of.is_empty() { score += 1.0; }
        fields += 1.0;
        if !proposal.physicality.is_empty() { score += 1.0; }
        fields += 1.0;
        if !proposal.role.is_empty() { score += 1.0; }
        fields += 1.0;

        if !proposal.alt_terms.is_empty() { score += 0.5; fields += 0.5; }
        if !proposal.relationships.is_empty() { score += 0.5; fields += 0.5; }

        (score / fields).min(1.0)
    }

    fn build_pr_body(
        &self,
        proposal: &NoteProposal,
        agent_ctx: &AgentContext,
        consistency: &ConsistencyReport,
        quality: f32,
    ) -> String {
        format!(
            r#"## Proposed Change

{task}

**Action**: Create new ontology note
**Agent**: {agent_type} ({agent_id})
**User**: {user_id}

## New Class

| Property | Value |
|----------|-------|
| IRI | `{iri}` |
| Term | {term} |
| Domain | {domain} |
| Parents | {parents} |

## Whelk Consistency Report

{consistency_status} {consistency_detail}

## Quality Assessment

- Quality Score: {quality:.2}/1.0
- Agent Confidence: {confidence:.2}/1.0
"#,
            task = agent_ctx.task_description,
            agent_type = agent_ctx.agent_type,
            agent_id = agent_ctx.agent_id,
            user_id = agent_ctx.user_id,
            iri = proposal.owl_class,
            term = proposal.preferred_term,
            domain = proposal.domain,
            parents = proposal.is_subclass_of.join(", "),
            consistency_status = if consistency.consistent { "**Consistent**" } else { "❌ **Inconsistent**" },
            consistency_detail = consistency.explanation.as_deref().unwrap_or("No logical contradictions"),
            quality = quality,
            confidence = agent_ctx.confidence,
        )
    }

    fn build_amend_pr_body(
        &self,
        target_iri: &str,
        amendment: &NoteAmendment,
        agent_ctx: &AgentContext,
        consistency: &ConsistencyReport,
    ) -> String {
        let mut changes = Vec::new();
        if amendment.update_definition.is_some() {
            changes.push("Updated definition".to_string());
        }
        for (rel_type, targets) in &amendment.add_relationships {
            for target in targets {
                changes.push(format!("Added {}: {}", rel_type, target));
            }
        }
        for (rel_type, targets) in &amendment.remove_relationships {
            for target in targets {
                changes.push(format!("Removed {}: {}", rel_type, target));
            }
        }

        format!(
            r#"## Proposed Amendment

{task}

**Action**: Amend existing note `{iri}`
**Agent**: {agent_type} ({agent_id})
**User**: {user_id}

## Changes

{changes}

## Whelk Consistency Report

{consistency_status}
"#,
            task = agent_ctx.task_description,
            iri = target_iri,
            agent_type = agent_ctx.agent_type,
            agent_id = agent_ctx.agent_id,
            user_id = agent_ctx.user_id,
            changes = changes.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n"),
            consistency_status = if consistency.consistent { "Consistent" } else { "❌ Inconsistent" },
        )
    }
}

// ---------------------------------------------------------------------------
// Pure projection helpers (asserted-graph triples + provenance seed)
// ---------------------------------------------------------------------------

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const OWL_CLASS: &str = "http://www.w3.org/2002/07/owl#Class";
const RDFS_SUBCLASS_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";

/// Pull the `contrasts-with` / `contrasts_with` targets out of a relationship map.
fn extract_contrasts(rels: &std::collections::HashMap<String, Vec<String>>) -> Vec<String> {
    let mut out = Vec::new();
    for key in CONTRASTS_WITH_KEYS {
        if let Some(targets) = rels.get(key) {
            out.extend(targets.iter().cloned());
        }
    }
    out
}

/// The plain current triples a create proposal projects into the asserted graph.
fn create_assert_triples(proposal: &NoteProposal) -> Vec<String> {
    let mut triples = vec![format!("<{}> <{}> <{}>", proposal.owl_class, RDF_TYPE, OWL_CLASS)];
    for parent in &proposal.is_subclass_of {
        triples.push(format!(
            "<{}> <{}> <{}>",
            proposal.owl_class, RDFS_SUBCLASS_OF, parent
        ));
    }
    triples
}

/// The plain current triples an amend proposal adds to the asserted graph.
fn amend_assert_triples(target_iri: &str, new_parents: &[String]) -> Vec<String> {
    new_parents
        .iter()
        .map(|parent| format!("<{}> <{}> <{}>", target_iri, RDFS_SUBCLASS_OF, parent))
        .collect()
}

/// A canonical provenance-content seed line for the receipt's provenance hash.
/// The full portable-reification quad set (ADR-049) is emitted by the T3
/// provenance writer and committed in Phase 2; this content-addresses the same
/// subject/agent/proposal tuple deterministically so the receipt is stable.
fn provenance_seed(proposal_id: &str, subject: &str, agent_iri: &str) -> Vec<String> {
    vec![format!(
        "assertion-version proposal:{} subject:{} attributedTo:{}",
        proposal_id, subject, agent_iri
    )]
}
