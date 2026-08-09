// src/bin/load_ontology.rs
//! Ontology Loader Binary
//!
//! Loads OWL ontology data from GitHub repository markdown files
//! and populates the Oxigraph quad-store (ADR-11).

use log::info;
use std::collections::HashMap;
use std::sync::Arc;

use visionclaw_server::adapters::OxigraphOntologyRepository;
use visionclaw_server::ports::ontology_repository::{
    AxiomType, OntologyRepository, OwlAxiom, OwlClass, OwlProperty, PropertyType,
};
use visionclaw_server::services::parsers::ontology_parser::OntologyParser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    info!("Starting ontology loader (Oxigraph backend, ADR-11)...");

    // 1. Initialize Oxigraph store
    use std::env;

    let data_dir = env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let oxigraph_path = std::path::Path::new(&data_dir).join("oxigraph");
    info!("Opening Oxigraph store at: {}", oxigraph_path.display());

    let ontology_repo = Arc::new(OxigraphOntologyRepository::open(&oxigraph_path).await?);

    info!("Oxigraph store opened successfully");

    // 2. Initialize parser
    let _parser = OntologyParser::new();

    // 3. Load sample ontology data for testing
    info!("Loading sample ontology classes...");

    // Create sample OWL classes for testing
    let sample_classes = vec![
        ("mv:Person", "Person", "A human individual", vec![]),
        (
            "mv:Company",
            "Company",
            "A business organization",
            vec!["mv:Concept".to_string()],
        ),
        (
            "mv:Project",
            "Project",
            "A collaborative endeavor",
            vec!["mv:Concept".to_string()],
        ),
        ("mv:Concept", "Concept", "An abstract idea", vec![]),
        (
            "mv:Technology",
            "Technology",
            "A technical tool or system",
            vec![],
        ),
    ];

    let mut _total_classes = 0;

    for (iri, label, desc, parents) in sample_classes {
        let class = OwlClass {
            iri: iri.to_string(),
            label: Some(label.to_string()),
            description: Some(desc.to_string()),
            parent_classes: parents,
            ..OwlClass::default()
        };

        ontology_repo.add_owl_class(&class).await?;
        _total_classes += 1;
        info!("Saved class: {} ({})", label, iri);
    }

    // 4. Create sample properties
    info!("Creating sample properties...");
    let prop = OwlProperty {
        iri: "mv:worksAt".to_string(),
        label: Some("works at".to_string()),
        property_type: PropertyType::ObjectProperty,
        domain: vec!["mv:Person".to_string()],
        range: vec!["mv:Company".to_string()],
        quality_score: None,
        authority_score: None,
        source_file: None,
    };
    ontology_repo.add_owl_property(&prop).await?;

    // 5. Create sample axioms
    info!("Creating sample axioms...");
    let axiom = OwlAxiom {
        id: None,
        axiom_type: AxiomType::SubClassOf,
        subject: "mv:Company".to_string(),
        object: "mv:Concept".to_string(),
        annotations: HashMap::new(),
    };
    ontology_repo.add_axiom(&axiom).await?;

    // 6. Verify data
    let all_classes = ontology_repo.get_classes().await?;
    info!("Ontology loaded successfully!");
    info!("Classes: {}", all_classes.len());
    info!("Stored in Oxigraph store at: {}", oxigraph_path.display());

    Ok(())
}
