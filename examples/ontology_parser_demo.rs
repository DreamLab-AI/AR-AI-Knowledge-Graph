// examples/ontology_parser_demo.rs
//! Demonstration of the enhanced ontology parser
//!
//! Run with: cargo run --example ontology_parser_demo

use visionclaw_server::services::parsers::ontology_parser::{OntologyBlock, OntologyParser};

fn main() {
    env_logger::init();

    let parser = OntologyParser::new();

    // Example 1: Complete ontology block with all tiers
    let example1 = r#"
# Large Language Models

- ### OntologyBlock
  id:: llm-ontology
  collapsed:: true

  - **Identification**
    - ontology:: true
    - term-id:: AI-0850
    - preferred-term:: Large Language Models
    - alt-terms:: [[LLM]], [[Foundation Models]], [[Generative AI Models]]
    - source-domain:: ai
    - status:: complete
    - public-access:: true
    - version:: 1.2.0
    - last-updated:: 2025-11-21
    - quality-score:: 0.92
    - cross-domain-links:: 47

  - **Definition**
    - definition:: A Large Language Model (LLM) is an artificial intelligence system based on deep neural networks (typically [[Transformer]] architectures) trained on vast text corpora to understand and generate human-like text, demonstrating emergent capabilities across diverse tasks including [[Natural Language Understanding]], [[Code Generation]], and [[Reasoning]].
    - maturity:: mature
    - source:: [[OpenAI Research]], [[Stanford AI Index]], [[ISO/IEC 23257:2021]]
    - authority-score:: 0.95
    - scope-note:: This definition focuses on autoregressive language models; excludes masked language models like BERT.

  - **Semantic Classification**
    - owl:class:: ai:LargeLanguageModel
    - owl:physicality:: VirtualEntity
    - owl:role:: Process
    - owl:inferred-class:: ai:VirtualProcess
    - belongsToDomain:: [[AI-GroundedDomain]], [[ComputationAndIntelligenceDomain]]
    - implementedInLayer:: [[ApplicationLayer]]

  - #### Relationships
    id:: llm-relationships
    - is-subclass-of:: [[Artificial Intelligence]], [[Neural Network Architecture]]
    - has-part:: [[Encoder]], [[Decoder]], [[Attention Mechanism]]
    - requires:: [[Training Data]], [[Computational Resources]], [[GPU Infrastructure]]
    - depends-on:: [[Transformer Architecture]], [[Attention Mechanism]]
    - enables:: [[Few-Shot Learning]], [[Zero-Shot Learning]], [[Text Generation]]
    - relates-to:: [[Natural Language Processing]], [[Prompt Engineering]]

  - #### CrossDomainBridges
    - bridges-to:: [[Blockchain Verification]] via enables
    - bridges-from:: [[Robotics Planning]] via requires

  - #### OWL Axioms
    id:: llm-owl-axioms
    collapsed:: true
    - ```clojure
      Prefix(:=<http://narrativegoldmine.com/ai#>)
      Prefix(owl:=<http://www.w3.org/2002/07/owl#>)
      Prefix(rdfs:=<http://www.w3.org/2000/01/rdf-schema#>)

      Ontology(<http://narrativegoldmine.com/ai/AI-0850>
        Declaration(Class(:LargeLanguageModel))
        SubClassOf(:LargeLanguageModel :NeuralNetworkArchitecture)
        AnnotationAssertion(rdfs:label :LargeLanguageModel "Large Language Model"@en)
        SubClassOf(:LargeLanguageModel
          ObjectSomeValuesFrom(:requires :TrainingData))
      )
      ```
"#;

    println!("=== Example 1: Complete Ontology Block ===\n");
    match parser.parse_enhanced(example1, "AI-0850-llm.md") {
        Ok(block) => {
            print_ontology_block(&block);
            let errors = block.validate();
            if errors.is_empty() {
                println!("\nValidation: PASSED");
            } else {
                println!("\nValidation: FAILED - {:?}", errors);
            }
        }
        Err(e) => println!("Error parsing: {}", e),
    }

    // Example 2: Minimal valid block
    let example2 = r#"
- ### OntologyBlock
  - ontology:: true
  - term-id:: BC-0001
  - preferred-term:: Blockchain
  - source-domain:: bc
  - status:: complete
  - public-access:: true
  - last-updated:: 2025-11-21
  - definition:: A distributed ledger technology that maintains a continuously growing list of records called blocks.
  - owl:class:: bc:Blockchain
  - owl:physicality:: VirtualEntity
  - owl:role:: Object

  - #### Relationships
    - is-subclass-of:: [[Distributed Ledger Technology]]
"#;

    println!("\n\n=== Example 2: Minimal Valid Block ===\n");
    match parser.parse_enhanced(example2, "BC-0001-blockchain.md") {
        Ok(block) => {
            print_ontology_block(&block);
            let errors = block.validate();
            if errors.is_empty() {
                println!("\nValidation: PASSED");
            } else {
                println!("\nValidation: FAILED - {:?}", errors);
            }
        }
        Err(e) => println!("Error parsing: {}", e),
    }

    // Example 3: Invalid block (missing required fields)
    let example3 = r#"
- ### OntologyBlock
  - term-id:: RB-0001
  - preferred-term:: Service Robot
  - definition:: A robot that performs tasks for human benefit.
"#;

    println!("\n\n=== Example 3: Invalid Block (Missing Required Fields) ===\n");
    match parser.parse_enhanced(example3, "RB-0001-service-robot.md") {
        Ok(block) => {
            let errors: Vec<String> = block.validate();
            println!("Term ID: {:?}", block.term_id);
            println!("Preferred Term: {:?}", block.preferred_term);
            println!("Definition: {:?}", block.definition);
            println!("\nValidation FAILED:");
            for (i, error) in errors.iter().enumerate() {
                println!("  {}. {}", i + 1, error);
            }
        }
        Err(e) => println!("Error parsing: {}", e),
    }
}

fn print_ontology_block(block: &OntologyBlock) {
    println!("File: {}", block.file_path);
    println!("Term ID: {:?}", block.term_id);
    println!("Preferred Term: {:?}", block.preferred_term);
    println!("Domain: {:?}", block.get_domain());
    println!("Full IRI: {:?}", block.get_full_iri());

    println!("\n--- Tier 1 (Required) ---");
    println!("  Status: {:?}", block.status);
    println!("  Public Access: {:?}", block.public_access);
    println!("  Last Updated: {:?}", block.last_updated);
    println!("  OWL Class: {:?}", block.owl_class);
    println!("  Physicality: {:?}", block.owl_physicality);
    println!("  Role: {:?}", block.owl_role);
    println!("  Parent Classes: {} items", block.is_subclass_of.len());
    if !block.is_subclass_of.is_empty() {
        for parent in &block.is_subclass_of {
            println!("    - {}", parent);
        }
    }

    println!("\n--- Tier 2 (Recommended) ---");
    if !block.alt_terms.is_empty() {
        println!("  Alt Terms: {:?}", block.alt_terms);
    }
    println!("  Version: {:?}", block.version);
    println!("  Quality Score: {:?}", block.quality_score);
    println!("  Maturity: {:?}", block.maturity);
    println!("  Authority Score: {:?}", block.authority_score);
    if !block.source.is_empty() {
        println!("  Sources: {:?}", block.source);
    }

    println!("\n--- Relationships ---");
    if !block.has_part.is_empty() {
        println!("  Has-Part: {:?}", block.has_part);
    }
    if !block.requires.is_empty() {
        println!("  Requires: {:?}", block.requires);
    }
    if !block.depends_on.is_empty() {
        println!("  Depends-On: {:?}", block.depends_on);
    }
    if !block.enables.is_empty() {
        println!("  Enables: {:?}", block.enables);
    }
    if !block.relates_to.is_empty() {
        println!("  Relates-To: {:?}", block.relates_to);
    }

    println!("\n--- Cross-Domain Bridges ---");
    if !block.bridges_to.is_empty() {
        println!("  Bridges-To: {:?}", block.bridges_to);
    }
    if !block.bridges_from.is_empty() {
        println!("  Bridges-From: {:?}", block.bridges_from);
    }

    println!("\n--- OWL Axioms ---");
    println!("  {} axiom block(s) found", block.owl_axioms.len());

    if !block.domain_extensions.is_empty() {
        println!("\n--- Domain Extensions ---");
        for (key, value) in &block.domain_extensions {
            println!("  {}: {}", key, value);
        }
    }

    if !block.other_relationships.is_empty() {
        println!("\n--- Other Relationships ---");
        for (key, values) in &block.other_relationships {
            println!("  {}: {:?}", key, values);
        }
    }
}
