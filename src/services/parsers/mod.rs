pub mod knowledge_graph_parser;
pub mod ontology_parser;

pub use knowledge_graph_parser::KnowledgeGraphParser;
pub use ontology_parser::OntologyParser;

#[derive(Debug, Clone)]
pub struct OntologyData {
    pub classes: Vec<crate::ports::ontology_repository::OwlClass>,

    pub properties: Vec<crate::ports::ontology_repository::OwlProperty>,

    pub axioms: Vec<crate::ports::ontology_repository::OwlAxiom>,
}

impl OntologyData {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            properties: Vec::new(),
            axioms: Vec::new(),
        }
    }

    pub fn with_capacity(classes: usize, properties: usize, axioms: usize) -> Self {
        Self {
            classes: Vec::with_capacity(classes),
            properties: Vec::with_capacity(properties),
            axioms: Vec::with_capacity(axioms),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.properties.is_empty() && self.axioms.is_empty()
    }

    pub fn total_elements(&self) -> usize {
        self.classes.len() + self.properties.len() + self.axioms.len()
    }
}

impl Default for OntologyData {
    fn default() -> Self {
        Self::new()
    }
}
