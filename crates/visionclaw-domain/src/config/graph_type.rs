//! ADR-2041 — graph-type vocabulary and the bounded `logseq` alias.
//!
//! The knowledge graph is named `knowledge`. The former name `logseq` is
//! accepted as a READ-ONLY alias for one release on any inbound value:
//! REST/WebSocket/query-string graph-type discriminators, `graphs` JSON object
//! keys, and dotted settings paths. Nothing ever *emits* `logseq`.
//!
//! This module is the single place the alias is spelled out. Deleting it (and
//! the `#[serde(alias = "logseq")]` on `GraphsSettings::knowledge`) is the whole
//! removal task for ADR-2041's `review_trigger`.

/// Normalise an inbound graph-type value to the canonical vocabulary.
///
/// Unknown values are passed through unchanged so callers can still reject them.
pub fn normalise_graph_type(graph: &str) -> &str {
    match graph {
        "logseq" | "knowledge" => "knowledge",
        "visionclaw" | "agent" | "bots" => "visionclaw",
        other => other,
    }
}

/// Look up the knowledge-graph entry inside an inbound `graphs` JSON value,
/// accepting the legacy `logseq` key.
pub fn knowledge_graph_value(graphs: &serde_json::Value) -> Option<&serde_json::Value> {
    graphs.get("knowledge").or_else(|| graphs.get("logseq"))
}

/// Whether an inbound `graphs` JSON object carries the knowledge graph under
/// either the canonical key or the legacy alias.
pub fn graphs_map_has_knowledge(graphs: &serde_json::Map<String, serde_json::Value>) -> bool {
    graphs.contains_key("knowledge") || graphs.contains_key("logseq")
}

/// Whether a dotted settings path addresses the knowledge graph, under either
/// the canonical segment or the legacy alias.
pub fn path_targets_knowledge_graph(path: &str) -> bool {
    path.contains(".graphs.knowledge.") || path.contains(".graphs.logseq.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_the_legacy_value() {
        assert_eq!(normalise_graph_type("logseq"), "knowledge");
        assert_eq!(normalise_graph_type("knowledge"), "knowledge");
        assert_eq!(normalise_graph_type("visionclaw"), "visionclaw");
        assert_eq!(normalise_graph_type("agent"), "visionclaw");
        assert_eq!(normalise_graph_type("bots"), "visionclaw");
        assert_eq!(normalise_graph_type("nonsense"), "nonsense");
    }

    #[test]
    fn json_lookups_accept_both_keys() {
        let legacy = serde_json::json!({ "logseq": { "physics": { "springK": 1 } } });
        let canonical = serde_json::json!({ "knowledge": { "physics": { "springK": 1 } } });
        assert_eq!(
            knowledge_graph_value(&legacy),
            legacy.get("logseq"),
            "legacy key must resolve"
        );
        assert_eq!(
            knowledge_graph_value(&canonical),
            canonical.get("knowledge")
        );
        assert!(knowledge_graph_value(&serde_json::json!({ "visionclaw": {} })).is_none());

        assert!(graphs_map_has_knowledge(legacy.as_object().unwrap()));
        assert!(graphs_map_has_knowledge(canonical.as_object().unwrap()));
        assert!(!graphs_map_has_knowledge(
            serde_json::json!({ "visionclaw": {} }).as_object().unwrap()
        ));
    }

    #[test]
    fn paths_match_under_either_segment() {
        assert!(path_targets_knowledge_graph(
            "visualisation.graphs.logseq.physics.springK"
        ));
        assert!(path_targets_knowledge_graph(
            "visualisation.graphs.knowledge.physics.springK"
        ));
        assert!(!path_targets_knowledge_graph(
            "visualisation.graphs.visionclaw.physics.springK"
        ));
    }
}
