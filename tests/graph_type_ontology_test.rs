use std::str::FromStr;
use visionclaw_server::models::graph_types::GraphType;

#[test]
fn test_ontology_graph_type_variant_exists() {
    // Verify the Ontology variant exists and can be constructed
    let ontology = GraphType::Ontology;
    assert_eq!(format!("{:?}", ontology), "Ontology");
}

#[test]
fn test_ontology_graph_type_display() {
    // Test Display trait for Ontology variant
    let ontology = GraphType::Ontology;
    assert_eq!(format!("{}", ontology), "ontology");
}

#[test]
fn test_ontology_graph_type_from_str() {
    // Test lowercase parsing
    assert_eq!(
        GraphType::from_str("ontology").unwrap(),
        GraphType::Ontology
    );

    // Test uppercase parsing (case-insensitive)
    assert_eq!(
        GraphType::from_str("Ontology").unwrap(),
        GraphType::Ontology
    );

    // Test mixed case
    assert_eq!(
        GraphType::from_str("ONTOLOGY").unwrap(),
        GraphType::Ontology
    );
}

#[test]
fn test_ontology_graph_type_serialization() {
    // Test JSON serialization
    let ontology = GraphType::Ontology;
    let serialized = serde_json::to_string(&ontology).unwrap();
    assert_eq!(serialized, "\"Ontology\"");

    // Test deserialization
    let deserialized: GraphType = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, GraphType::Ontology);
}

#[test]
fn test_all_graph_types_parsing() {
    // Verify all graph types can be parsed from strings
    assert_eq!(
        GraphType::from_str("standard").unwrap(),
        GraphType::Standard
    );
    assert_eq!(
        GraphType::from_str("multi-agent").unwrap(),
        GraphType::MultiAgent
    );
    assert_eq!(
        GraphType::from_str("multiagent").unwrap(),
        GraphType::MultiAgent
    );
    assert_eq!(
        GraphType::from_str("force-directed").unwrap(),
        GraphType::ForceDirected
    );
    assert_eq!(
        GraphType::from_str("forcedirected").unwrap(),
        GraphType::ForceDirected
    );
    assert_eq!(
        GraphType::from_str("hierarchical").unwrap(),
        GraphType::Hierarchical
    );
    assert_eq!(GraphType::from_str("network").unwrap(), GraphType::Network);
    assert_eq!(
        GraphType::from_str("ontology").unwrap(),
        GraphType::Ontology
    );
}

#[test]
fn test_invalid_graph_type_parsing() {
    // Verify invalid graph types return errors
    assert!(GraphType::from_str("invalid").is_err());
    assert!(GraphType::from_str("").is_err());
    assert!(GraphType::from_str("unknown-type").is_err());
}

#[test]
fn test_graph_type_clone() {
    // Verify GraphType can be cloned
    let original = GraphType::Ontology;
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_graph_type_equality() {
    // Test PartialEq implementation
    assert_eq!(GraphType::Ontology, GraphType::Ontology);
    assert_ne!(GraphType::Ontology, GraphType::Standard);
    assert_ne!(GraphType::Ontology, GraphType::MultiAgent);
}

#[test]
fn test_default_graph_type() {
    // Verify default is still Standard (no breaking change)
    assert_eq!(GraphType::default(), GraphType::Standard);
}

#[test]
fn test_graph_type_roundtrip_serialization() {
    // Test all variants serialize and deserialize correctly
    let types = vec![
        GraphType::Standard,
        GraphType::MultiAgent,
        GraphType::ForceDirected,
        GraphType::Hierarchical,
        GraphType::Network,
        GraphType::Ontology,
    ];

    for graph_type in types {
        let json = serde_json::to_string(&graph_type).unwrap();
        let deserialized: GraphType = serde_json::from_str(&json).unwrap();
        assert_eq!(graph_type, deserialized);
    }
}

#[test]
fn test_ontology_in_collection() {
    // Test that Ontology variant works in collections
    let mut types = vec![GraphType::Standard, GraphType::MultiAgent];
    types.push(GraphType::Ontology);

    assert!(types.contains(&GraphType::Ontology));
    assert_eq!(types.len(), 3);
    assert_eq!(types[2], GraphType::Ontology);
}

#[test]
fn test_graph_type_match_patterns() {
    // Test match expressions work with Ontology variant
    let graph_type = GraphType::Ontology;

    let result = match graph_type {
        GraphType::Standard => "standard",
        GraphType::MultiAgent => "multi-agent",
        GraphType::ForceDirected => "force-directed",
        GraphType::Hierarchical => "hierarchical",
        GraphType::Network => "network",
        GraphType::Ontology => "ontology",
    };

    assert_eq!(result, "ontology");
}

#[test]
fn test_graph_type_debug_output() {
    // Verify Debug trait provides useful output
    let ontology = GraphType::Ontology;
    let debug_str = format!("{:?}", ontology);
    assert_eq!(debug_str, "Ontology");
}

#[test]
fn test_backward_compatibility() {
    // Verify existing graph types still work (no breaking changes)
    let standard = GraphType::Standard;
    let multi_agent = GraphType::MultiAgent;
    let force_directed = GraphType::ForceDirected;

    assert_eq!(format!("{}", standard), "standard");
    assert_eq!(format!("{}", multi_agent), "multi-agent");
    assert_eq!(format!("{}", force_directed), "force-directed");
}

#[test]
fn test_case_insensitive_parsing_all_types() {
    // Verify all types support case-insensitive parsing
    assert!(GraphType::from_str("STANDARD").is_ok());
    assert!(GraphType::from_str("MULTI-AGENT").is_ok());
    assert!(GraphType::from_str("FORCE-DIRECTED").is_ok());
    assert!(GraphType::from_str("HIERARCHICAL").is_ok());
    assert!(GraphType::from_str("NETWORK").is_ok());
    assert!(GraphType::from_str("ONTOLOGY").is_ok());
}
