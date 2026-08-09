// src/application/ontology/directives.rs
//! Ontology Domain - Write Operations (Directives)
//!
//! All directives for modifying ontology state following CQRS patterns.

use hexser::{Directive, DirectiveHandler, HexResult, Hexserror};
use std::sync::Arc;

use visionclaw_domain::models::graph::GraphData;
use visionclaw_domain::ports::ontology_repository::{
    InferenceResults, OntologyRepository, OwlAxiom, OwlClass, OwlProperty,
};

// ============================================================================
// ADD OWL CLASS
// ============================================================================

#[derive(Debug, Clone)]
pub struct AddOwlClass {
    pub class: OwlClass,
}

impl Directive for AddOwlClass {
    fn validate(&self) -> HexResult<()> {
        if self.class.iri.is_empty() {
            return Err(Hexserror::validation("Class IRI cannot be empty"));
        }
        Ok(())
    }
}

pub struct AddOwlClassHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl AddOwlClassHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<AddOwlClass> for AddOwlClassHandler {
    fn handle(&self, directive: AddOwlClass) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing AddOwlClass directive: iri={}",
                directive.class.iri
            );

            self.repository
                .add_owl_class(&directive.class)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!("OWL class added successfully: iri={}", directive.class.iri);
            Ok(())
        })
    }
}

// ============================================================================
// UPDATE OWL CLASS
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdateOwlClass {
    pub class: OwlClass,
}

impl Directive for UpdateOwlClass {
    fn validate(&self) -> HexResult<()> {
        if self.class.iri.is_empty() {
            return Err(Hexserror::validation("Class IRI cannot be empty"));
        }
        Ok(())
    }
}

pub struct UpdateOwlClassHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl UpdateOwlClassHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<UpdateOwlClass> for UpdateOwlClassHandler {
    fn handle(&self, directive: UpdateOwlClass) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing UpdateOwlClass directive: iri={}",
                directive.class.iri
            );

            let existing = self
                .repository
                .get_owl_class(&directive.class.iri)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            if existing.is_none() {
                return Err(Hexserror::not_found("OWL class", &directive.class.iri));
            }

            self.repository
                .add_owl_class(&directive.class)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!(
                "OWL class updated successfully: iri={}",
                directive.class.iri
            );
            Ok(())
        })
    }
}

// ============================================================================
// REMOVE OWL CLASS
// ============================================================================

#[derive(Debug, Clone)]
pub struct RemoveOwlClass {
    pub iri: String,
}

impl Directive for RemoveOwlClass {
    fn validate(&self) -> HexResult<()> {
        if self.iri.is_empty() {
            return Err(Hexserror::validation("IRI cannot be empty"));
        }
        Ok(())
    }
}

pub struct RemoveOwlClassHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl RemoveOwlClassHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<RemoveOwlClass> for RemoveOwlClassHandler {
    fn handle(&self, directive: RemoveOwlClass) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!("Executing RemoveOwlClass directive: iri={}", directive.iri);

            let existing = self
                .repository
                .get_owl_class(&directive.iri)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            if existing.is_none() {
                return Err(Hexserror::not_found("OWL class", &directive.iri));
            }

            self.repository
                .remove_owl_class(&directive.iri)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!("OWL class removed successfully: iri={}", directive.iri);
            Ok(())
        })
    }
}

// ============================================================================
// ADD OWL PROPERTY
// ============================================================================

#[derive(Debug, Clone)]
pub struct AddOwlProperty {
    pub property: OwlProperty,
}

impl Directive for AddOwlProperty {
    fn validate(&self) -> HexResult<()> {
        if self.property.iri.is_empty() {
            return Err(Hexserror::validation("Property IRI cannot be empty"));
        }
        Ok(())
    }
}

pub struct AddOwlPropertyHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl AddOwlPropertyHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<AddOwlProperty> for AddOwlPropertyHandler {
    fn handle(&self, directive: AddOwlProperty) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing AddOwlProperty directive: iri={}",
                directive.property.iri
            );

            self.repository
                .add_owl_property(&directive.property)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!(
                "OWL property added successfully: iri={}",
                directive.property.iri
            );
            Ok(())
        })
    }
}

// ============================================================================
// UPDATE OWL PROPERTY
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdateOwlProperty {
    pub property: OwlProperty,
}

impl Directive for UpdateOwlProperty {
    fn validate(&self) -> HexResult<()> {
        if self.property.iri.is_empty() {
            return Err(Hexserror::validation("Property IRI cannot be empty"));
        }
        Ok(())
    }
}

pub struct UpdateOwlPropertyHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl UpdateOwlPropertyHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<UpdateOwlProperty> for UpdateOwlPropertyHandler {
    fn handle(&self, directive: UpdateOwlProperty) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing UpdateOwlProperty directive: iri={}",
                directive.property.iri
            );

            let existing = self
                .repository
                .get_owl_property(&directive.property.iri)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            if existing.is_none() {
                return Err(Hexserror::not_found(
                    "OWL property",
                    &directive.property.iri,
                ));
            }

            self.repository
                .add_owl_property(&directive.property)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!(
                "OWL property updated successfully: iri={}",
                directive.property.iri
            );
            Ok(())
        })
    }
}

// ============================================================================
// ADD AXIOM
// ============================================================================

#[derive(Debug, Clone)]
pub struct AddAxiom {
    pub axiom: OwlAxiom,
}

impl Directive for AddAxiom {
    fn validate(&self) -> HexResult<()> {
        if self.axiom.subject.is_empty() {
            return Err(Hexserror::validation("Axiom subject cannot be empty"));
        }
        if self.axiom.object.is_empty() {
            return Err(Hexserror::validation("Axiom object cannot be empty"));
        }
        Ok(())
    }
}

pub struct AddAxiomHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl AddAxiomHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<AddAxiom> for AddAxiomHandler {
    fn handle(&self, directive: AddAxiom) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing AddAxiom directive: type={:?}, subject={}, object={}",
                directive.axiom.axiom_type,
                directive.axiom.subject,
                directive.axiom.object
            );

            self.repository
                .add_axiom(&directive.axiom)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!("Axiom added successfully");
            Ok(())
        })
    }
}

// ============================================================================
// REMOVE AXIOM
// ============================================================================

#[derive(Debug, Clone)]
pub struct RemoveAxiom {
    pub axiom_id: u64,
}

impl Directive for RemoveAxiom {
    fn validate(&self) -> HexResult<()> {
        if self.axiom_id == 0 {
            return Err(Hexserror::validation("Axiom ID must be greater than 0"));
        }
        Ok(())
    }
}

pub struct RemoveAxiomHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl RemoveAxiomHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<RemoveAxiom> for RemoveAxiomHandler {
    fn handle(&self, directive: RemoveAxiom) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!("Executing RemoveAxiom directive: id={}", directive.axiom_id);

            self.repository
                .remove_axiom(directive.axiom_id)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!("Axiom removed successfully: id={}", directive.axiom_id);
            Ok(())
        })
    }
}

// ============================================================================
// STORE INFERENCE RESULTS
// ============================================================================

#[derive(Debug, Clone)]
pub struct StoreInferenceResults {
    pub results: InferenceResults,
}

impl Directive for StoreInferenceResults {
    fn validate(&self) -> HexResult<()> {
        Ok(())
    }
}

pub struct StoreInferenceResultsHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl StoreInferenceResultsHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<StoreInferenceResults> for StoreInferenceResultsHandler {
    fn handle(&self, directive: StoreInferenceResults) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing StoreInferenceResults directive: {} inferred axioms",
                directive.results.inferred_axioms.len()
            );

            self.repository
                .store_inference_results(&directive.results)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!("Inference results stored successfully");
            Ok(())
        })
    }
}

// ============================================================================
// SAVE ONTOLOGY GRAPH
// ============================================================================

#[derive(Debug, Clone)]
pub struct SaveOntologyGraph {
    pub graph: GraphData,
}

impl Directive for SaveOntologyGraph {
    fn validate(&self) -> HexResult<()> {
        Ok(())
    }
}

pub struct SaveOntologyGraphHandler {
    repository: Arc<dyn OntologyRepository>,
}

impl SaveOntologyGraphHandler {
    pub fn new(repository: Arc<dyn OntologyRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<SaveOntologyGraph> for SaveOntologyGraphHandler {
    fn handle(&self, directive: SaveOntologyGraph) -> HexResult<()> {
        tokio::runtime::Handle::current().block_on(async {
            log::info!(
                "Executing SaveOntologyGraph directive: {} nodes, {} edges",
                directive.graph.nodes.len(),
                directive.graph.edges.len()
            );

            self.repository
                .save_ontology_graph(&directive.graph)
                .await
                .map_err(|e| Hexserror::port("E_REPO_001", &format!("{}", e)))?;

            log::info!("Ontology graph saved successfully");
            Ok(())
        })
    }
}
