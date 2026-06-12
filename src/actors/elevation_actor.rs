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
//! Boot is env-gated: the actor starts only when `ELEVATION_ACTOR_ENABLED=1`
//! and `FORUM_RELAY_URL` + a panel secret are configured. The ACSP panel
//! identity must be registered in the relay's `agent_registry` (the pubkey is
//! logged at startup for the admin).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use log::{error, info, warn};
use serde_json::json;

use crate::ports::knowledge_graph_repository::KnowledgeGraphRepository;
use crate::services::acsp::{
    build_action_request, build_panel_definition, build_panel_state, AcspClient, ActionPriority,
    ActionRequest, CaseCategory, CaseDecision, CaseSpec, SubjectKind,
};
use crate::services::acsp::events::{
    ActionDef, ActionStyle, FieldDef, FieldType, LayoutHint, PanelDefinition, PanelSchema,
};
use crate::services::github_pr_service::GitHubPRService;
use crate::types::ontology_tools::AgentContext;

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

#[derive(Debug, Clone)]
struct PendingCase {
    label: String,
    file_path: String,
    draft: String,
}

pub struct ElevationActor {
    kg_repo: Arc<dyn KnowledgeGraphRepository>,
    acsp: Option<Arc<AcspClient>>,
    panel_secret: String,
    forum_relay_url: String,
    /// Open broker cases awaiting a human decision, keyed by case id.
    pending: HashMap<String, PendingCase>,
    /// Frontier labels already cased/decided this session (skip list).
    seen: HashSet<String>,
    elevated_count: u32,
    rejected_count: u32,
    last_pr_url: Option<String>,
}

impl ElevationActor {
    pub fn new(kg_repo: Arc<dyn KnowledgeGraphRepository>) -> Option<Self> {
        let enabled = std::env::var("ELEVATION_ACTOR_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !enabled {
            return None;
        }
        let forum_relay_url = std::env::var("FORUM_RELAY_URL").ok()?;
        let panel_secret = std::env::var("ACSP_PANEL_NOSTR_PRIVKEY")
            .or_else(|_| std::env::var("VISIONCLAW_NOSTR_PRIVKEY"))
            .ok()?;
        Some(Self {
            kg_repo,
            acsp: None,
            panel_secret,
            forum_relay_url,
            pending: HashMap::new(),
            seen: HashSet::new(),
            elevated_count: 0,
            rejected_count: 0,
            last_pr_url: None,
        })
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
            "last_pr_url": self.last_pr_url,
        })
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
    }
}

impl Handler<RunCycle> for ElevationActor {
    type Result = ();

    fn handle(&mut self, _msg: RunCycle, ctx: &mut Self::Context) {
        let Some(acsp) = self.acsp.clone() else {
            return;
        };
        if self.pending.len() >= MAX_OPEN_CASES {
            return;
        }
        let kg = self.kg_repo.clone();
        let skip: HashSet<String> = self
            .seen
            .iter()
            .cloned()
            .chain(self.pending.values().map(|p| p.label.clone()))
            .collect();
        let budget = MAX_OPEN_CASES - self.pending.len();

        ctx.spawn(
            actix::fut::wrap_future::<_, Self>(async move {
                let graph = match kg.load_graph().await {
                    Ok(g) => g,
                    Err(e) => {
                        warn!("[Elevation] load_graph failed: {e}");
                        return (Vec::new(), 0);
                    }
                };
                let candidates = select_frontier_candidates(&graph, &skip, budget);
                let frontier_size = graph
                    .nodes
                    .iter()
                    .filter(|n| n.node_type.as_deref() == Some("owl_class"))
                    .filter(|n| !n.metadata.contains_key("source_file"))
                    .count();

                let mut opened: Vec<(String, PendingCase)> = Vec::new();
                for c in candidates {
                    let (file_path, draft) = draft_class_page(&c);
                    let name = canonical_name(&c.label);
                    let case_id = format!("{CASE_PREFIX}{}", slugify(&name));
                    let spec = CaseSpec {
                        case_id: case_id.clone(),
                        title: format!("Elevate: {name}"),
                        priority: ActionPriority::Medium,
                        category: CaseCategory::KnowledgeEnrichment,
                        subject_kind: SubjectKind::AutomationProposal,
                        subject_id: format!("urn:ngm:class:{}", slugify(&name)),
                        request: ActionRequest {
                            fields: json!({
                                "name": name,
                                "domain": c.domain,
                                "referenced_by": c.referenced_by,
                                "definition_preview": draft.lines().find(|l| l.contains("definition")).unwrap_or("").trim(),
                                "file_path": file_path,
                                "degree": c.degree,
                            }),
                            reasoning: Some(format!(
                                "Frontier concept with {} axiom references — most-cited unauthored class in the current graph snapshot.",
                                c.degree
                            )),
                            context_url: None,
                        },
                    };
                    match acsp.publish(&build_action_request(&spec)).await {
                        Ok(_) => {
                            info!("[Elevation] opened case {case_id} for '{}'", c.label);
                            opened.push((
                                case_id,
                                PendingCase {
                                    label: c.label.clone(),
                                    file_path,
                                    draft,
                                },
                            ));
                        }
                        Err(e) => warn!("[Elevation] case publish failed for '{}': {e}", c.label),
                    }
                }
                (opened, frontier_size)
            })
            .map(|(opened, frontier_size), act, ctx| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use visionclaw_domain::models::edge::Edge;
    use visionclaw_domain::models::graph::GraphData;
    use visionclaw_domain::models::node::Node;

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
