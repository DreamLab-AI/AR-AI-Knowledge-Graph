//! Graph type definitions for multi-agent systems and semantic forces

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum GraphType {
    #[default]
    Standard,

    MultiAgent,

    ForceDirected,

    Hierarchical,

    Network,

    Ontology,
}

/// Semantic node types for type-aware physics and pathfinding
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum NodeType {
    /// Generic untyped node
    #[default]
    Generic,
    /// Person or individual entity
    Person,
    /// Organization or group
    Organization,
    /// Project or initiative
    Project,
    /// Task or action item
    Task,
    /// Concept or abstract idea
    Concept,
    /// OWL class definition
    Class,
    /// OWL individual instance
    Individual,
    /// Custom user-defined type
    Custom(String),
}

/// Semantic edge types for relationship-aware algorithms
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum EdgeType {
    /// Generic untyped edge
    #[default]
    Generic,
    /// Dependency relationship
    Dependency,
    /// Hierarchical parent-child relationship
    Hierarchy,
    /// General association
    Association,
    /// Sequential ordering
    Sequence,
    /// OWL subClassOf relationship
    SubClassOf,
    /// OWL instanceOf relationship
    InstanceOf,
    /// Custom user-defined type
    Custom(String),
}

impl std::fmt::Display for GraphType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphType::Standard => write!(f, "standard"),
            GraphType::MultiAgent => write!(f, "multi-agent"),
            GraphType::ForceDirected => write!(f, "force-directed"),
            GraphType::Hierarchical => write!(f, "hierarchical"),
            GraphType::Network => write!(f, "network"),
            GraphType::Ontology => write!(f, "ontology"),
        }
    }
}

impl std::str::FromStr for GraphType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(GraphType::Standard),
            "multi-agent" | "multiagent" => Ok(GraphType::MultiAgent),
            "force-directed" | "forcedirected" => Ok(GraphType::ForceDirected),
            "hierarchical" => Ok(GraphType::Hierarchical),
            "network" => Ok(GraphType::Network),
            "ontology" => Ok(GraphType::Ontology),
            _ => Err(format!("Unknown graph type: {}", s)),
        }
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Generic => write!(f, "generic"),
            NodeType::Person => write!(f, "person"),
            NodeType::Organization => write!(f, "organization"),
            NodeType::Project => write!(f, "project"),
            NodeType::Task => write!(f, "task"),
            NodeType::Concept => write!(f, "concept"),
            NodeType::Class => write!(f, "class"),
            NodeType::Individual => write!(f, "individual"),
            NodeType::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "generic" => Ok(NodeType::Generic),
            "person" => Ok(NodeType::Person),
            "organization" | "org" => Ok(NodeType::Organization),
            "project" => Ok(NodeType::Project),
            "task" => Ok(NodeType::Task),
            "concept" => Ok(NodeType::Concept),
            "class" => Ok(NodeType::Class),
            "individual" => Ok(NodeType::Individual),
            _ => Ok(NodeType::Custom(s.to_string())),
        }
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Generic => write!(f, "generic"),
            EdgeType::Dependency => write!(f, "dependency"),
            EdgeType::Hierarchy => write!(f, "hierarchy"),
            EdgeType::Association => write!(f, "association"),
            EdgeType::Sequence => write!(f, "sequence"),
            EdgeType::SubClassOf => write!(f, "subClassOf"),
            EdgeType::InstanceOf => write!(f, "instanceOf"),
            EdgeType::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl std::str::FromStr for EdgeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "generic" => Ok(EdgeType::Generic),
            "dependency" | "depends" => Ok(EdgeType::Dependency),
            "hierarchy" | "parent-child" => Ok(EdgeType::Hierarchy),
            "association" | "assoc" => Ok(EdgeType::Association),
            "sequence" | "seq" => Ok(EdgeType::Sequence),
            "subclassof" | "subclass" => Ok(EdgeType::SubClassOf),
            "instanceof" | "instance" => Ok(EdgeType::InstanceOf),
            _ => Ok(EdgeType::Custom(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // --- GraphType ---

    #[test]
    fn graph_type_default_is_standard() {
        assert_eq!(GraphType::default(), GraphType::Standard);
    }

    #[test]
    fn graph_type_display_all_variants() {
        assert_eq!(GraphType::Standard.to_string(), "standard");
        assert_eq!(GraphType::MultiAgent.to_string(), "multi-agent");
        assert_eq!(GraphType::ForceDirected.to_string(), "force-directed");
        assert_eq!(GraphType::Hierarchical.to_string(), "hierarchical");
        assert_eq!(GraphType::Network.to_string(), "network");
        assert_eq!(GraphType::Ontology.to_string(), "ontology");
    }

    #[test]
    fn graph_type_from_str_valid() {
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
            GraphType::from_str("ontology").unwrap(),
            GraphType::Ontology
        );
    }

    #[test]
    fn graph_type_from_str_unknown_returns_err() {
        assert!(GraphType::from_str("bogus").is_err());
    }

    #[test]
    fn graph_type_serde_roundtrip() {
        let t = GraphType::Hierarchical;
        let json = serde_json::to_string(&t).unwrap();
        let back: GraphType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    // --- NodeType ---

    #[test]
    fn node_type_default_is_generic() {
        assert_eq!(NodeType::default(), NodeType::Generic);
    }

    #[test]
    fn node_type_display_all_named_variants() {
        assert_eq!(NodeType::Generic.to_string(), "generic");
        assert_eq!(NodeType::Person.to_string(), "person");
        assert_eq!(NodeType::Organization.to_string(), "organization");
        assert_eq!(NodeType::Project.to_string(), "project");
        assert_eq!(NodeType::Task.to_string(), "task");
        assert_eq!(NodeType::Concept.to_string(), "concept");
        assert_eq!(NodeType::Class.to_string(), "class");
        assert_eq!(NodeType::Individual.to_string(), "individual");
        assert_eq!(
            NodeType::Custom("my-type".to_string()).to_string(),
            "my-type"
        );
    }

    #[test]
    fn node_type_from_str_aliases() {
        assert_eq!(NodeType::from_str("org").unwrap(), NodeType::Organization);
        assert_eq!(NodeType::from_str("person").unwrap(), NodeType::Person);
        // Unknown strings become Custom, never an error
        assert!(matches!(
            NodeType::from_str("unknown_xyz").unwrap(),
            NodeType::Custom(_)
        ));
    }

    #[test]
    fn node_type_serde_roundtrip() {
        let t = NodeType::Custom("owl-thing".to_string());
        let json = serde_json::to_string(&t).unwrap();
        let back: NodeType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    // --- EdgeType ---

    #[test]
    fn edge_type_default_is_generic() {
        assert_eq!(EdgeType::default(), EdgeType::Generic);
    }

    #[test]
    fn edge_type_display_all_named_variants() {
        assert_eq!(EdgeType::Generic.to_string(), "generic");
        assert_eq!(EdgeType::Dependency.to_string(), "dependency");
        assert_eq!(EdgeType::Hierarchy.to_string(), "hierarchy");
        assert_eq!(EdgeType::Association.to_string(), "association");
        assert_eq!(EdgeType::Sequence.to_string(), "sequence");
        assert_eq!(EdgeType::SubClassOf.to_string(), "subClassOf");
        assert_eq!(EdgeType::InstanceOf.to_string(), "instanceOf");
        assert_eq!(EdgeType::Custom("x".to_string()).to_string(), "x");
    }

    #[test]
    fn edge_type_from_str_aliases() {
        assert_eq!(EdgeType::from_str("depends").unwrap(), EdgeType::Dependency);
        assert_eq!(
            EdgeType::from_str("parent-child").unwrap(),
            EdgeType::Hierarchy
        );
        assert_eq!(EdgeType::from_str("assoc").unwrap(), EdgeType::Association);
        assert_eq!(EdgeType::from_str("seq").unwrap(), EdgeType::Sequence);
        assert_eq!(
            EdgeType::from_str("subclassof").unwrap(),
            EdgeType::SubClassOf
        );
        assert_eq!(
            EdgeType::from_str("instance").unwrap(),
            EdgeType::InstanceOf
        );
        // Unknown becomes Custom
        assert!(matches!(
            EdgeType::from_str("weird").unwrap(),
            EdgeType::Custom(_)
        ));
    }

    #[test]
    fn edge_type_serde_roundtrip() {
        let t = EdgeType::SubClassOf;
        let json = serde_json::to_string(&t).unwrap();
        let back: EdgeType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
