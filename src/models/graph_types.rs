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
