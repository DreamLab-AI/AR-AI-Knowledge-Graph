/// Test to verify Node and Edge structs match unified_schema.sql
///
/// Schema Requirements:
/// - Node: id, metadata_id, label, x, y, z, vx, vy, vz, mass, owl_class_iri
/// - Edge: id, source, target, weight, edge_type, owl_property_iri, metadata
use visionclaw_server::models::{edge::Edge, node::Node};

#[test]
fn test_node_has_all_schema_fields() {
    // Create a node with all required fields
    let node = Node::new("test-node".to_string())
        .with_label("Test Node".to_string())
        .with_position(1.0, 2.0, 3.0)
        .with_velocity(0.1, 0.2, 0.3)
        .with_mass(5.0)
        .with_owl_class_iri("http://example.org/ontology#TestClass".to_string());

    // Verify core fields
    assert!(node.id > 0);
    assert_eq!(node.metadata_id, "test-node");
    assert_eq!(node.label, "Test Node");

    // Verify physics fields match schema
    assert_eq!(node.x, Some(1.0));
    assert_eq!(node.y, Some(2.0));
    assert_eq!(node.z, Some(3.0));
    assert_eq!(node.vx, Some(0.1));
    assert_eq!(node.vy, Some(0.2));
    assert_eq!(node.vz, Some(0.3));
    assert_eq!(node.mass, Some(5.0));

    // Verify OWL ontology linkage
    assert_eq!(
        node.owl_class_iri,
        Some("http://example.org/ontology#TestClass".to_string())
    );

    // Verify BinaryNodeData is also updated
    assert_eq!(node.data.x, 1.0);
    assert_eq!(node.data.y, 2.0);
    assert_eq!(node.data.z, 3.0);
    assert_eq!(node.data.vx, 0.1);
    assert_eq!(node.data.vy, 0.2);
    assert_eq!(node.data.vz, 0.3);
}

#[test]
fn test_node_mass_getter() {
    let node1 = Node::new("test1".to_string()).with_mass(10.0);
    assert_eq!(node1.get_mass(), 10.0);

    let node2 = Node::new("test2".to_string());
    assert_eq!(node2.get_mass(), 1.0); // Default mass
}

#[test]
fn test_node_setters_update_both_fields() {
    let mut node = Node::new("test".to_string());

    // Set position
    node.set_x(5.0);
    node.set_y(6.0);
    node.set_z(7.0);

    // Verify both struct fields and BinaryNodeData are updated
    assert_eq!(node.x, Some(5.0));
    assert_eq!(node.y, Some(6.0));
    assert_eq!(node.z, Some(7.0));
    assert_eq!(node.data.x, 5.0);
    assert_eq!(node.data.y, 6.0);
    assert_eq!(node.data.z, 7.0);

    // Set velocity
    node.set_vx(0.5);
    node.set_vy(0.6);
    node.set_vz(0.7);

    assert_eq!(node.vx, Some(0.5));
    assert_eq!(node.vy, Some(0.6));
    assert_eq!(node.vz, Some(0.7));
    assert_eq!(node.data.vx, 0.5);
    assert_eq!(node.data.vy, 0.6);
    assert_eq!(node.data.vz, 0.7);

    // Set mass
    node.set_mass(3.5);
    assert_eq!(node.mass, Some(3.5));
}

#[test]
fn test_edge_has_all_schema_fields() {
    use std::collections::HashMap;

    let mut metadata = HashMap::new();
    metadata.insert("key1".to_string(), "value1".to_string());

    // Create an edge with all required fields
    let edge = Edge::new(1, 2, 1.5)
        .with_edge_type("SubClassOf".to_string())
        .with_owl_property_iri("http://www.w3.org/2000/01/rdf-schema#subClassOf".to_string())
        .with_metadata(metadata);

    // Verify core fields
    assert_eq!(edge.id, "1-2");
    assert_eq!(edge.source, 1);
    assert_eq!(edge.target, 2);
    assert_eq!(edge.weight, 1.5);

    // Verify edge type
    assert_eq!(edge.edge_type, Some("SubClassOf".to_string()));

    // Verify OWL property linkage
    assert_eq!(
        edge.owl_property_iri,
        Some("http://www.w3.org/2000/01/rdf-schema#subClassOf".to_string())
    );

    // Verify metadata
    assert!(edge.metadata.is_some());
    let meta = edge.metadata.unwrap();
    assert_eq!(meta.get("key1"), Some(&"value1".to_string()));
}

#[test]
fn test_edge_builder_methods() {
    let edge = Edge::new(10, 20, 2.0)
        .add_metadata("prop1".to_string(), "val1".to_string())
        .add_metadata("prop2".to_string(), "val2".to_string());

    assert!(edge.metadata.is_some());
    let meta = edge.metadata.unwrap();
    assert_eq!(meta.len(), 2);
    assert_eq!(meta.get("prop1"), Some(&"val1".to_string()));
    assert_eq!(meta.get("prop2"), Some(&"val2".to_string()));
}

#[test]
fn test_node_default_initialization() {
    let node = Node::new("test-default".to_string());

    // Verify defaults are set
    assert!(node.id > 0);
    assert_eq!(node.metadata_id, "test-default");
    assert!(node.x.is_some()); // Position should be initialized
    assert!(node.y.is_some());
    assert!(node.z.is_some());
    assert_eq!(node.vx, Some(0.0)); // Velocity starts at 0
    assert_eq!(node.vy, Some(0.0));
    assert_eq!(node.vz, Some(0.0));
    assert_eq!(node.mass, Some(1.0)); // Default mass
    assert!(node.owl_class_iri.is_none()); // No OWL class by default
}

#[test]
fn test_edge_default_initialization() {
    let edge = Edge::new(1, 2, 1.0);

    assert_eq!(edge.id, "1-2");
    assert_eq!(edge.source, 1);
    assert_eq!(edge.target, 2);
    assert_eq!(edge.weight, 1.0);
    assert!(edge.edge_type.is_none());
    assert!(edge.owl_property_iri.is_none());
    assert!(edge.metadata.is_none());
}

#[test]
fn test_node_serialization_compatibility() {
    // Test that nodes can be serialized/deserialized
    let node = Node::new("serialization-test".to_string())
        .with_position(10.0, 20.0, 30.0)
        .with_mass(2.5)
        .with_owl_class_iri("http://example.org/Class1".to_string());

    let json = serde_json::to_string(&node).expect("Failed to serialize node");
    let deserialized: Node = serde_json::from_str(&json).expect("Failed to deserialize node");

    assert_eq!(node.id, deserialized.id);
    assert_eq!(node.metadata_id, deserialized.metadata_id);
    assert_eq!(node.x, deserialized.x);
    assert_eq!(node.y, deserialized.y);
    assert_eq!(node.z, deserialized.z);
    assert_eq!(node.mass, deserialized.mass);
    assert_eq!(node.owl_class_iri, deserialized.owl_class_iri);
}

#[test]
fn test_edge_serialization_compatibility() {
    let edge = Edge::new(5, 10, 3.0)
        .with_owl_property_iri("http://example.org/property1".to_string())
        .with_edge_type("relates_to".to_string());

    let json = serde_json::to_string(&edge).expect("Failed to serialize edge");
    let deserialized: Edge = serde_json::from_str(&json).expect("Failed to deserialize edge");

    assert_eq!(edge.id, deserialized.id);
    assert_eq!(edge.source, deserialized.source);
    assert_eq!(edge.target, deserialized.target);
    assert_eq!(edge.weight, deserialized.weight);
    assert_eq!(edge.owl_property_iri, deserialized.owl_property_iri);
    assert_eq!(edge.edge_type, deserialized.edge_type);
}
