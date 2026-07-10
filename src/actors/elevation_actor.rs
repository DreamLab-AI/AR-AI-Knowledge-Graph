//! ElevationActor — the flagship ACSP agentic actor (ADR-110).
//!
//! Closes the knowledge-elevation loop: the formal ontology's *frontier* —
//! classes referenced by axioms but never authored as pages (`owl_class`
//! stubs) — is a ready-made work queue. The actor selects the most-referenced
//! frontier concepts, drafts a canonical Class page for each, and opens a
//! `knowledge_enrichment` broker case on the forum governance page with a
//! custom control surface carrying everything a human needs to judge:
//! proposed canonical name, slug, inferred domain, draft definition and the
//! referencing classes. An `approve` decision (kind 31403) commits the draft
//! to the corpus repo as a PR via [`GitHubPRService`]; `reject` skips the
//! candidate for the session.
//!
//! Boot is env-gated. Per ADR-130 Decision 2 (REC-2), the gate now defaults ON
//! in dev/staging and stays opt-in in production: `ELEVATION_ACTOR_ENABLED`
//! defaults to `true` unless `APP_ENV`/`NODE_ENV` is `production`, and an
//! explicit `ELEVATION_ACTOR_ENABLED=0`/`=1` always wins. The actor still
//! additionally requires `FORUM_RELAY_URL` + a panel secret to publish and sign
//! ACSP events, so a dev box without a relay configured stays dormant even with
//! the gate open. The ACSP panel identity must be registered in the relay's
//! `agent_registry` (the pubkey is logged at startup for the admin).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use log::{error, info, warn};
use serde_json::json;

use crate::actors::elevation_voice::{
    harvest_mentions, parse_elevation_intent, ConceptIndex, VoiceDemandLedger,
};
use crate::adapters::sqlite_enrichment_repository::{
    EnrichmentProposal as StoredProposal, SqliteEnrichmentRepository, StoredDecision,
};
use crate::ports::knowledge_graph_repository::KnowledgeGraphRepository;
use crate::services::acsp::{
    build_action_request, build_panel_definition, build_panel_state, AcspClient, ActionPriority,
    ActionRequest, CaseCategory, CaseDecision, CaseSpec, SubjectKind,
};
use crate::services::acsp::events::{
    ActionDef, ActionStyle, FieldDef, FieldType, LayoutHint, PanelDefinition, PanelSchema,
};
use crate::services::github_pr_service::GitHubPRService;
use crate::services::speech_service::SpeechService;
use crate::types::ontology_tools::AgentContext;
use crate::types::speech::SpeechOptions;

/// NIP-33 panel id (the `d` tag) for the elevation control surface.
const PANEL_ID: &str = "vc-elevation";
/// Case-id namespace; the decision subscription filters on this prefix.
const CASE_PREFIX: &str = "vc-elev-";
/// How many broker cases may be open at once.
const MAX_OPEN_CASES: usize = 5;
/// Candidate scan cadence.
const CYCLE_INTERVAL: Duration = Duration::from_secs(600);

#[derive(Message)]
#[rtype(result = "()")]
struct RunCycle;

#[derive(Message)]
#[rtype(result = "()")]
struct Decision(CaseDecision);

/// One transcription line from the local Whisper STT stream.
#[derive(Message)]
#[rtype(result = "()")]
struct VoiceTranscript(String);

#[derive(Debug, Clone)]
struct PendingCase {
    label: String,
    file_path: String,
    draft: String,
}

pub struct ElevationActor {
    kg_repo: Arc<dyn KnowledgeGraphRepository>,
    /// Durable projection of the actor's working set. Opened cases are persisted
    /// here as `state=pending` at case-open so `/api/broker/inbox` shows a
    /// pending proposal BEFORE any decision (previously the inbox was empty until
    /// the decide-time stub), and the Decision handler reconciles the row so the
    /// REST and actor views agree. The in-memory `pending` map stays the working
    /// set; this store is the durable projection (ADR-130 Decision 2 / gap-close).
    enrichment_repo: Arc<SqliteEnrichmentRepository>,
    acsp: Option<Arc<AcspClient>>,
    /// Local Kokoro TTS / Whisper STT bridge: transcripts guide candidate
    /// selection; the actor speaks confirmations back into the session.
    speech: Option<Arc<SpeechService>>,
    panel_secret: String,
    forum_relay_url: String,
    /// Open broker cases awaiting a human decision, keyed by case id.
    pending: HashMap<String, PendingCase>,
    /// Frontier labels already cased/decided this session (skip list).
    seen: HashSet<String>,
    /// Conversational demand (decaying) — the PRIMARY elevation signal.
    voice: VoiceDemandLedger,
    /// Concept lookup over the latest graph snapshot's elevatable labels.
    concept_index: Arc<ConceptIndex>,
    elevated_count: u32,
    rejected_count: u32,
    voice_case_count: u32,
    last_pr_url: Option<String>,
}

/// Is this a production deployment? Production is signalled by `APP_ENV` or
/// `NODE_ENV` set to `production` (the prod docker-compose profile sets
/// `NODE_ENV=production`; `main.rs` reads `APP_ENV=production` for the same
/// intent). Anything else — dev, staging, an unset env — is non-production.
fn is_production_env() -> bool {
    is_production_from(
        std::env::var("APP_ENV").ok(),
        std::env::var("NODE_ENV").ok(),
    )
}

/// Pure production check over the two environment signals, factored out so the
/// dev-default-ON / prod-opt-in policy is unit-testable without mutating
/// process-global env state.
fn is_production_from(app_env: Option<String>, node_env: Option<String>) -> bool {
    let is_prod = |v: &Option<String>| {
        v.as_deref()
            .map(|s| s.eq_ignore_ascii_case("production"))
            .unwrap_or(false)
    };
    is_prod(&app_env) || is_prod(&node_env)
}

impl ElevationActor {
    pub fn new(
        kg_repo: Arc<dyn KnowledgeGraphRepository>,
        enrichment_repo: Arc<SqliteEnrichmentRepository>,
        speech: Option<Arc<SpeechService>>,
    ) -> Option<Self> {
        // REC-2 / ADR-130 Decision 2: the case queue only carries real cases if
        // this consumer runs, so default the gate ON in dev/staging while
        // keeping production opt-in. An explicit env value always wins.
        let enabled = match std::env::var("ELEVATION_ACTOR_ENABLED") {
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
            kg_repo,
            enrichment_repo,
            acsp: None,
            speech,
            panel_secret,
            forum_relay_url,
            pending: HashMap::new(),
            seen: HashSet::new(),
            voice: VoiceDemandLedger::new(),
            concept_index: Arc::new(ConceptIndex::build(std::iter::empty())),
            elevated_count: 0,
            rejected_count: 0,
            voice_case_count: 0,
            last_pr_url: None,
        })
    }

    /// Speak a short confirmation into the immersive session via local Kokoro
    /// TTS. Fire-and-forget: voice feedback must never block case handling.
    fn speak(&self, text: String) {
        if let Some(speech) = self.speech.clone() {
            tokio::spawn(async move {
                if let Err(e) = speech.text_to_speech(text, SpeechOptions::default()).await {
                    warn!("[Elevation] TTS confirmation failed: {e}");
                }
            });
        }
    }

    fn panel_definition() -> PanelDefinition {
        PanelDefinition {
            title: "Knowledge Elevation".into(),
            description: "Frontier ontology concepts proposed for formalisation. \
                          Approve to commit a draft Class page to the corpus as a PR."
                .into(),
            version: "1.0.0".into(),
            schema: PanelSchema::ActionInbox,
            fields: vec![
                FieldDef {
                    name: "name".into(),
                    field_type: FieldType::String,
                    label: "Proposed class".into(),
                },
                FieldDef {
                    name: "domain".into(),
                    field_type: FieldType::String,
                    label: "Domain".into(),
                },
                FieldDef {
                    name: "referenced_by".into(),
                    field_type: FieldType::Json,
                    label: "Referencing classes".into(),
                },
                FieldDef {
                    name: "definition".into(),
                    field_type: FieldType::String,
                    label: "Draft definition".into(),
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
                    label: "Elevate".into(),
                    style: ActionStyle::Primary,
                },
                ActionDef {
                    id: "reject".into(),
                    label: "Skip".into(),
                    style: ActionStyle::Secondary,
                },
            ],
            layout: LayoutHint::InboxTable,
            capabilities: vec![],
            refresh_secs: 60,
        }
    }

    fn state_snapshot(&self, frontier_size: usize) -> serde_json::Value {
        json!({
            "frontier_size": frontier_size,
            "open_cases": self.pending.len(),
            "elevated": self.elevated_count,
            "rejected": self.rejected_count,
            "voice_cases": self.voice_case_count,
            "voice_guided": self.speech.is_some(),
            "last_pr_url": self.last_pr_url,
        })
    }

    /// Open a broker case for a candidate (shared by the cycle path and the
    /// explicit voice-intent path). Returns the case spec to publish plus the
    /// pending record.
    fn case_for(
        c: &FrontierCandidate,
        voice: Option<&crate::actors::elevation_voice::VoiceDemand>,
        priority: ActionPriority,
    ) -> (CaseSpec, PendingCase) {
        let (file_path, draft) = draft_class_page(c);
        let name = canonical_name(&c.label);
        let case_id = format!("{CASE_PREFIX}{}", slugify(&name));
        let mut fields = json!({
            "name": name,
            "domain": c.domain,
            "referenced_by": c.referenced_by,
            "file_path": file_path.clone(),
            "degree": c.degree,
        });
        let reasoning = match voice {
            Some(v) => {
                fields["voice_mentions"] = json!(v.mentions);
                fields["voice_excerpts"] = json!(v.excerpts);
                if !v.speakers.is_empty() {
                    fields["voice_speakers"] = json!(v.speakers);
                }
                format!(
                    "Raised in conversation: {} mention(s) in recent voice sessions \
                     (latest: \"{}\"). Graph degree {}.",
                    v.mentions,
                    v.excerpts.last().cloned().unwrap_or_default(),
                    c.degree
                )
            }
            None => format!(
                "Frontier concept with {} axiom references — most-cited unauthored class in the current graph snapshot.",
                c.degree
            ),
        };
        let spec = CaseSpec {
            case_id,
            title: format!("Elevate: {name}"),
            priority,
            category: CaseCategory::KnowledgeEnrichment,
            subject_kind: SubjectKind::AutomationProposal,
            subject_id: format!("urn:ngm:class:{}", slugify(&name)),
            request: ActionRequest {
                fields,
                reasoning: Some(reasoning),
                context_url: None,
            },
        };
        let pending = PendingCase {
            label: c.label.clone(),
            file_path,
            draft,
        };
        (spec, pending)
    }

    /// Build the durable `state=pending` projection row for a freshly opened
    /// case. The `proposal_json` carries the fields the WS-12 broker-inbox
    /// projection reads (`target_path`, `content`, `enrichment_type`,
    /// `reasoning_summary`, `proposed_by`) so the pending proposal renders in the
    /// inbox before any decision. `created_at`/`updated_at` are `0` here — the
    /// store stamps `unixepoch()` on write.
    fn pending_proposal(spec: &CaseSpec, pending: &PendingCase) -> StoredProposal {
        StoredProposal {
            case_id: spec.case_id.clone(),
            category: Some(spec.category.as_tag_value().to_string()),
            source_iri: Some(spec.subject_id.clone()),
            proposal_json: json!({
                "target_path": pending.file_path,
                "content": pending.draft,
                "enrichment_type": "class_elevation",
                "reasoning_summary": spec.request.reasoning,
                "proposed_by": format!("elevation-{}", slugify(&pending.label)),
                "title": spec.title,
            }),
            status: "pending".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// One frontier candidate: an `owl_class` stub referenced by axioms but never
/// authored, ranked by graph degree.
#[derive(Debug, Clone)]
pub struct FrontierCandidate {
    pub label: String,
    pub degree: usize,
    pub domain: String,
    pub referenced_by: Vec<String>,
}

/// Pure candidate selection over a loaded graph snapshot — unit-testable
/// without actors or relays. Frontier = `owl_class` nodes with no
/// `source_file` metadata (nothing authored them). Degree ranks importance;
/// `referenced_by` carries up to 8 neighbouring labels for the case panel;
/// the domain is the majority `source_domain` among referencing neighbours.
pub fn select_frontier_candidates(
    graph: &visionclaw_domain::models::graph::GraphData,
    skip: &HashSet<String>,
    limit: usize,
) -> Vec<FrontierCandidate> {
    let mut degree: HashMap<u32, usize> = HashMap::new();
    let mut neighbours: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in &graph.edges {
        *degree.entry(e.source).or_insert(0) += 1;
        *degree.entry(e.target).or_insert(0) += 1;
        neighbours.entry(e.source).or_default().push(e.target);
        neighbours.entry(e.target).or_default().push(e.source);
    }
    let by_id: HashMap<u32, &visionclaw_domain::models::node::Node> =
        graph.nodes.iter().map(|n| (n.id, n)).collect();

    let mut candidates: Vec<FrontierCandidate> = graph
        .nodes
        .iter()
        .filter(|n| n.node_type.as_deref() == Some("owl_class"))
        .filter(|n| !n.metadata.contains_key("source_file"))
        .filter(|n| !n.label.trim().is_empty())
        .filter(|n| !skip.contains(&n.label))
        .map(|n| {
            let nbrs = neighbours.get(&n.id).cloned().unwrap_or_default();
            let mut domains: HashMap<String, usize> = HashMap::new();
            let mut referenced_by: Vec<String> = Vec::new();
            for nb in nbrs.iter().take(64) {
                if let Some(node) = by_id.get(nb) {
                    if referenced_by.len() < 8 && !node.label.trim().is_empty() {
                        referenced_by.push(node.label.clone());
                    }
                    if let Some(d) = node.metadata.get("source_domain") {
                        *domains.entry(d.clone()).or_insert(0) += 1;
                    }
                }
            }
            let domain = domains
                .into_iter()
                .max_by_key(|(_, c)| *c)
                .map(|(d, _)| d)
                .unwrap_or_else(|| "infrastructure".into());
            FrontierCandidate {
                label: n.label.clone(),
                degree: degree.get(&n.id).copied().unwrap_or(0),
                domain,
                referenced_by,
            }
        })
        .collect();

    candidates.sort_by(|a, b| b.degree.cmp(&a.degree).then(a.label.cmp(&b.label)));
    candidates.truncate(limit);
    candidates
}

/// Canonical Title Case page name from a frontier label (the corpus
/// convention: descriptive Title Case filenames, e.g. "Fairness Auditing
/// Tools"). Slug derivation mirrors the server slugifier.
pub fn canonical_name(label: &str) -> String {
    label
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true;
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Draft a canonical Class page for a frontier concept. Same JSON-LD shape as
/// the existing elevated corpus pages (v2 context, `urn:ngm:class:` identity,
/// maturity `draft` so the quality layers treat it honestly).
pub fn draft_class_page(c: &FrontierCandidate) -> (String, String) {
    let name = canonical_name(&c.label);
    let slug = slugify(&name);
    let refs = c
        .referenced_by
        .iter()
        .map(|r| format!("\"{}\"", r.replace('"', "'")))
        .collect::<Vec<_>>()
        .join(", ");
    let definition = format!(
        "Draft elevation of the frontier concept '{}': referenced by {} graph relationships \
         (including {}) but not yet formally authored. Refine this definition during review.",
        name,
        c.degree,
        c.referenced_by
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    let content = format!(
        "# {name}\n\
         ```json-ld\n\
         {{\n\
         \x20 \"@context\": \"https://narrativegoldmine.com/ns/v2.jsonld\",\n\
         \x20 \"@id\": \"urn:ngm:class:{slug}\",\n\
         \x20 \"@type\": \"Class\",\n\
         \x20 \"label\": \"{name}\",\n\
         \x20 \"definition\": \"{definition}\",\n\
         \x20 \"domain\": \"{domain}\",\n\
         \x20 \"maturity\": \"draft\",\n\
         \x20 \"qualityScore\": 0.5,\n\
         \x20 \"subClassOf\": [],\n\
         \x20 \"vc:referencedBy\": [{refs}]\n\
         }}\n\
         ```\n",
        name = name,
        slug = slug,
        definition = definition.replace('"', "'"),
        domain = c.domain,
        refs = refs,
    );
    (format!("mainKnowledgeGraph/pages/{name}.md"), content)
}

impl Actor for ElevationActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[Elevation] actor starting (panel '{PANEL_ID}', case prefix '{CASE_PREFIX}')");
        let secret = self.panel_secret.clone();
        let relay = self.forum_relay_url.clone();
        let addr = ctx.address();

        // Connect the ACSP client, publish the panel definition, and start the
        // decision subscription. All async; results land back via messages.
        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                match AcspClient::connect(&secret, &relay).await {
                    Ok(client) => {
                        let client = Arc::new(client);
                        let def = build_panel_definition(PANEL_ID, &Self::panel_definition());
                        if let Err(e) = client.publish(&def).await {
                            warn!("[Elevation] panel definition publish failed: {e}");
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
                        error!("[Elevation] ACSP connect failed: {e}; actor idle");
                        None
                    }
                }
            })
            .map(|client, act, ctx| {
                act.acsp = client;
                if act.acsp.is_some() {
                    ctx.address().do_send(RunCycle);
                }
            }),
        );

        ctx.run_interval(CYCLE_INTERVAL, |_act, ctx| {
            ctx.address().do_send(RunCycle);
        });

        // Voice guidance: forward every local-Whisper transcription line into
        // the actor. Conversation is the primary elevation signal; the stream
        // is fire-and-forget and lossy (broadcast lag is tolerated).
        if let Some(speech) = self.speech.clone() {
            let addr = ctx.address();
            tokio::spawn(async move {
                let mut rx = speech.subscribe_to_transcriptions();
                loop {
                    match rx.recv().await {
                        Ok(line) => addr.do_send(VoiceTranscript(line)),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("[Elevation] transcription stream lagged ({n} lines skipped)");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            info!("[Elevation] voice guidance active (local Whisper STT → demand ledger; Kokoro TTS confirmations)");
        }
    }
}

impl Handler<VoiceTranscript> for ElevationActor {
    type Result = ();

    fn handle(&mut self, VoiceTranscript(line): VoiceTranscript, ctx: &mut Self::Context) {
        let now = std::time::Instant::now();

        // Explicit spoken command — jumps the queue entirely.
        if let Some(phrase) = parse_elevation_intent(&line) {
            let label = self
                .concept_index
                .lookup(&phrase)
                .map(str::to_string)
                .unwrap_or(phrase);
            if self.seen.contains(&label)
                || self.pending.values().any(|p| p.label == label)
            {
                self.speak(format!("{label} is already in elevation review."));
                return;
            }
            self.voice.note(&label, &line, "", now);
            let Some(acsp) = self.acsp.clone() else { return };
            let candidate = FrontierCandidate {
                label: label.clone(),
                degree: 0,
                domain: "infrastructure".into(),
                referenced_by: Vec::new(),
            };
            let demand = self.voice.demand(&label).cloned();
            let (spec, pending) =
                Self::case_for(&candidate, demand.as_ref(), ActionPriority::High);
            let case_id = spec.case_id.clone();
            let proposal = Self::pending_proposal(&spec, &pending);
            let repo = self.enrichment_repo.clone();
            ctx.spawn(
                actix::fut::wrap_future::<_, Self>(async move {
                    let published =
                        acsp.publish(&build_action_request(&spec)).await.map(|_| ());
                    // Durable projection of the voice-commanded open case.
                    if published.is_ok() {
                        if let Err(e) = repo.create_or_update(&proposal).await {
                            warn!("[Elevation] voice pending-case persist failed: {e}");
                        }
                    }
                    published
                })
                .map(move |result, act, _ctx| match result {
                    Ok(()) => {
                        info!("[Elevation] voice-commanded case {case_id} for '{}'", pending.label);
                        act.voice_case_count += 1;
                        act.seen.insert(pending.label.clone());
                        let spoken = pending.label.clone();
                        act.pending.insert(case_id, pending);
                        act.speak(format!(
                            "Opened an elevation case for {spoken}. Review it on the governance page."
                        ));
                    }
                    Err(e) => warn!("[Elevation] voice case publish failed: {e}"),
                }),
            );
            return;
        }

        // Ambient mentions feed the demand ledger that ranks the next cycle.
        for label in harvest_mentions(&line, &self.concept_index) {
            self.voice.note(&label, &line, "", now);
        }
    }
}

impl Handler<RunCycle> for ElevationActor {
    type Result = ();

    fn handle(&mut self, _msg: RunCycle, ctx: &mut Self::Context) {
        let Some(acsp) = self.acsp.clone() else {
            return;
        };
        let now = std::time::Instant::now();
        self.voice.prune(now);
        if self.pending.len() >= MAX_OPEN_CASES {
            return;
        }
        let kg = self.kg_repo.clone();
        let repo = self.enrichment_repo.clone();
        let skip: HashSet<String> = self
            .seen
            .iter()
            .cloned()
            .chain(self.pending.values().map(|p| p.label.clone()))
            .collect();
        let budget = MAX_OPEN_CASES - self.pending.len();

        // Snapshot the conversational demand so the async block needs no
        // access to the ledger. Voice is the PRIMARY ranking signal; degree
        // breaks ties and carries the queue when nobody is talking.
        let voice_scores: HashMap<String, f32> = skip
            .iter()
            .map(|l| (l.clone(), 0.0))
            .chain(
                self.voice
                    .labels()
                    .map(|l| (l.to_string(), self.voice.score(l, now))),
            )
            .collect();
        let voice_demands: HashMap<String, crate::actors::elevation_voice::VoiceDemand> = self
            .voice
            .labels()
            .filter_map(|l| self.voice.demand(l).map(|d| (l.to_string(), d.clone())))
            .collect();

        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                let graph = match kg.load_graph().await {
                    Ok(g) => g,
                    Err(e) => {
                        warn!("[Elevation] load_graph failed: {e}");
                        return (Vec::new(), 0, Vec::new());
                    }
                };
                // Wide candidate pool, then voice-first ordering.
                let mut candidates = select_frontier_candidates(&graph, &skip, budget * 8);
                candidates.sort_by(|a, b| {
                    let va = voice_scores.get(&a.label).copied().unwrap_or(0.0);
                    let vb = voice_scores.get(&b.label).copied().unwrap_or(0.0);
                    vb.partial_cmp(&va)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(b.degree.cmp(&a.degree))
                        .then(a.label.cmp(&b.label))
                });
                candidates.truncate(budget);

                let frontier_size = graph
                    .nodes
                    .iter()
                    .filter(|n| n.node_type.as_deref() == Some("owl_class"))
                    .filter(|n| !n.metadata.contains_key("source_file"))
                    .count();

                // Refresh the concept index from this snapshot: frontier stubs
                // plus working pages are the vocabulary voice mentions match.
                let index_labels: Vec<String> = graph
                    .nodes
                    .iter()
                    .filter(|n| {
                        matches!(n.node_type.as_deref(), Some("owl_class") | Some("page"))
                    })
                    .map(|n| n.label.clone())
                    .collect();

                let mut opened: Vec<(String, PendingCase)> = Vec::new();
                for c in candidates {
                    let voice = voice_demands.get(&c.label);
                    let priority = if voice.is_some() {
                        ActionPriority::High
                    } else {
                        ActionPriority::Medium
                    };
                    let (spec, pending) = ElevationActor::case_for(&c, voice, priority);
                    let case_id = spec.case_id.clone();
                    match acsp.publish(&build_action_request(&spec)).await {
                        Ok(_) => {
                            // Durable projection: persist the open case as
                            // state=pending so /api/broker/inbox shows it BEFORE
                            // any decision. Failure is logged loudly, never fatal
                            // to the working set.
                            let proposal = ElevationActor::pending_proposal(&spec, &pending);
                            if let Err(e) = repo.create_or_update(&proposal).await {
                                warn!(
                                    "[Elevation] pending-case persist failed for {case_id}: {e}"
                                );
                            }
                            info!(
                                "[Elevation] opened case {case_id} for '{}' (voice={})",
                                c.label,
                                voice.is_some()
                            );
                            opened.push((case_id, pending));
                        }
                        Err(e) => warn!("[Elevation] case publish failed for '{}': {e}", c.label),
                    }
                }
                (opened, frontier_size, index_labels)
            })
            .map(|(opened, frontier_size, index_labels), act, ctx| {
                if !index_labels.is_empty() {
                    act.concept_index = Arc::new(ConceptIndex::build(
                        index_labels.iter().map(String::as_str),
                    ));
                }
                for (case_id, pending) in opened {
                    act.seen.insert(pending.label.clone());
                    act.pending.insert(case_id, pending);
                }
                act.publish_state(ctx, frontier_size);
            }),
        );
    }
}

impl Handler<Decision> for ElevationActor {
    type Result = ();

    fn handle(&mut self, Decision(d): Decision, ctx: &mut Self::Context) {
        let Some(case) = self.pending.remove(&d.case_id) else {
            return; // replayed/foreign decision
        };
        info!(
            "[Elevation] case {} decided '{}' by {} — {}",
            d.case_id, d.action, d.responder_pubkey, d.reasoning
        );

        // Reconcile the durable projection: record the human decision so the REST
        // inbox/history and the actor's working set agree (the pending row opened
        // at case-open transitions to its decided status atomically here). The PR
        // side-effect below is separate — the elevation "commit" is the merged PR,
        // not the Oxigraph :summary write, so `writeback_committed` stays false.
        {
            let repo = self.enrichment_repo.clone();
            let stored = decision_record(&d);
            let case_id = d.case_id.clone();
            ctx.spawn(
                actix::fut::wrap_future::<_, Self>(async move {
                    if let Err(e) = repo.record_decision(&stored).await {
                        warn!("[Elevation] decision reconcile persist failed for {case_id}: {e}");
                    }
                })
                .map(|_, _, _| ()),
            );
        }

        match d.action.as_str() {
            "approve" => {
                let pr = GitHubPRService::new();
                let label = case.label.clone();
                ctx.spawn(
                    actix::fut::wrap_future::<_, Self>(async move {
                        let agent_ctx = AgentContext {
                            agent_id: format!("elevation-{}", slugify(&label)),
                            agent_type: "elevation".into(),
                            task_description: format!(
                                "ACSP-approved elevation of frontier concept '{label}'"
                            ),
                            session_id: None,
                            confidence: 0.5,
                            user_id: "acsp-governance".into(),
                        };
                        pr.create_ontology_pr(
                            &case.file_path,
                            &case.draft,
                            &format!("feat(ontology): elevate '{label}' (draft)"),
                            &format!(
                                "Draft Class page for frontier concept **{label}**, approved \
                                 via the forum governance panel (ACSP case). Definition is a \
                                 draft — refine during PR review.\n\n🤖 Generated by Claude Code"
                            ),
                            &agent_ctx,
                        )
                        .await
                    })
                    .map(|result, act, ctx| {
                        match result {
                            Ok(url) => {
                                act.elevated_count += 1;
                                act.last_pr_url = Some(url.clone());
                                info!("[Elevation] PR created: {url}");
                            }
                            Err(e) => error!("[Elevation] PR creation failed: {e}"),
                        }
                        act.publish_state(ctx, 0);
                        // Refill the case window.
                        ctx.address().do_send(RunCycle);
                    }),
                );
            }
            _ => {
                self.rejected_count += 1;
                self.publish_state(ctx, 0);
                ctx.address().do_send(RunCycle);
            }
        }
    }
}

impl ElevationActor {
    fn publish_state(&self, ctx: &mut Context<Self>, frontier_size: usize) {
        let Some(acsp) = self.acsp.clone() else {
            return;
        };
        let state = self.state_snapshot(frontier_size);
        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                if let Err(e) = acsp.publish(&build_panel_state(PANEL_ID, &state)).await {
                    warn!("[Elevation] panel state publish failed: {e}");
                }
            })
            .map(|_, _, _| ()),
        );
    }
}

/// Build the durable [`StoredDecision`] reconciliation record from a forum
/// [`CaseDecision`] (kind-31403). The responding admin's pubkey attributes the
/// decision when it is a canonical x-only hex key; a non-hex key downgrades to
/// unattributed (never an error). `writeback_committed` stays `false`: the
/// elevation "commit" is the merged ontology PR (tracked separately), not the
/// enrichment-decide Oxigraph `:summary` write.
fn decision_record(d: &CaseDecision) -> StoredDecision {
    let attributed = crate::uri::is_pubkey_hex(&d.responder_pubkey);
    let owner_did = if attributed {
        crate::uri::did_nostr(&d.responder_pubkey).ok()
    } else {
        None
    };
    let proposal_urn = if attributed {
        crate::uri::kg(
            &d.responder_pubkey,
            format!("enrichment-proposal:{}", d.case_id),
        )
        .ok()
    } else {
        None
    };
    let activity_urn = crate::uri::execution(format!(
        "elevation-decide:{}:{}:{}",
        d.case_id, d.action, d.responder_pubkey
    ));
    let writeback_triggered =
        crate::adapters::sqlite_enrichment_repository::status_for_outcome(&d.action) == "approved";
    let decided_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    StoredDecision {
        case_id: d.case_id.clone(),
        outcome: d.action.clone(),
        attributed,
        broker_pubkey: Some(d.responder_pubkey.clone()),
        reasoning: Some(d.reasoning.clone()),
        writeback_triggered,
        writeback_committed: false,
        activity_urn,
        proposal_urn,
        owner_did,
        decided_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use visionclaw_domain::models::edge::Edge;
    use visionclaw_domain::models::graph::GraphData;
    use visionclaw_domain::models::node::Node;

    #[test]
    fn production_gate_defaults_dev_on_prod_off() {
        // Unset env → non-production → ElevationActor defaults ON.
        assert!(!is_production_from(None, None));
        // Either signal at `production` (any case) → production → opt-in only.
        assert!(is_production_from(Some("production".into()), None));
        assert!(is_production_from(None, Some("Production".into())));
        assert!(is_production_from(Some("PRODUCTION".into()), Some("development".into())));
        // Dev/staging values are not production.
        assert!(!is_production_from(Some("development".into()), Some("development".into())));
        assert!(!is_production_from(Some("staging".into()), None));
    }

    fn node(id: u32, label: &str, node_type: &str, authored: bool, domain: Option<&str>) -> Node {
        let mut n = Node::default();
        n.id = id;
        n.label = label.into();
        n.metadata_id = slugify(label);
        n.node_type = Some(node_type.into());
        if authored {
            n.metadata
                .insert("source_file".into(), format!("{label}.md"));
        }
        if let Some(d) = domain {
            n.metadata.insert("source_domain".into(), d.into());
        }
        n
    }

    fn edge(source: u32, target: u32) -> Edge {
        Edge {
            id: format!("{source}_{target}"),
            source,
            target,
            weight: 1.0,
            edge_type: None,
            metadata: None,
            owl_property_iri: None,
        }
    }

    fn frontier_graph() -> GraphData {
        let mut g = GraphData::new();
        g.nodes = vec![
            node(1, "finality mechanism", "owl_class", false, None),
            node(2, "search space definition", "owl_class", false, None),
            node(3, "Authored Class", "owl_class", true, Some("blockchain")),
            node(4, "Consensus Layer", "ontology_node", true, Some("blockchain")),
            node(5, "Some Page", "page", true, Some("infrastructure")),
        ];
        // 'finality mechanism' degree 3 (hub), 'search space definition' degree 1.
        g.edges = vec![edge(4, 1), edge(3, 1), edge(5, 1), edge(4, 2)];
        g
    }

    #[test]
    fn frontier_selection_ranks_by_degree_and_skips_authored() {
        let g = frontier_graph();
        let picked = select_frontier_candidates(&g, &HashSet::new(), 10);
        assert_eq!(picked.len(), 2, "only unauthored owl_class stubs qualify");
        assert_eq!(picked[0].label, "finality mechanism");
        assert_eq!(picked[0].degree, 3);
        assert_eq!(picked[0].domain, "blockchain");
        assert!(picked[0]
            .referenced_by
            .contains(&"Consensus Layer".to_string()));
    }

    #[test]
    fn frontier_selection_honours_skip_list_and_limit() {
        let g = frontier_graph();
        let mut skip = HashSet::new();
        skip.insert("finality mechanism".to_string());
        let picked = select_frontier_candidates(&g, &skip, 10);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].label, "search space definition");

        let limited = select_frontier_candidates(&g, &HashSet::new(), 1);
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn canonical_name_title_cases() {
        assert_eq!(canonical_name("finality mechanism"), "Finality Mechanism");
        assert_eq!(canonical_name("ar content positioning"), "Ar Content Positioning");
    }

    #[test]
    fn draft_page_has_canonical_identity_and_draft_maturity() {
        let c = FrontierCandidate {
            label: "finality mechanism".into(),
            degree: 7,
            domain: "blockchain".into(),
            referenced_by: vec!["Consensus Layer".into(), "Bitcoin Proof-of-Work Protocol".into()],
        };
        let (path, content) = draft_class_page(&c);
        assert_eq!(path, "mainKnowledgeGraph/pages/Finality Mechanism.md");
        assert!(content.contains("\"@id\": \"urn:ngm:class:finality-mechanism\""));
        assert!(content.contains("\"@type\": \"Class\""));
        assert!(content.contains("\"maturity\": \"draft\""));
        assert!(content.contains("\"domain\": \"blockchain\""));
        // The JSON-LD block must parse.
        let block = content
            .split("```json-ld\n")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(block).expect("json-ld parses");
        assert_eq!(parsed["label"], "Finality Mechanism");
    }

    #[test]
    fn slugify_matches_corpus_convention() {
        assert_eq!(slugify("Finality Mechanism"), "finality-mechanism");
        assert_eq!(slugify("3D and 4D"), "3d-and-4d");
    }
}
