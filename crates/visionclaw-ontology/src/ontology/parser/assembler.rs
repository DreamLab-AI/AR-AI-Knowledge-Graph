use anyhow::Result;

pub struct OntologyAssembler {
    header: String,
    axiom_blocks: Vec<String>,
}

impl OntologyAssembler {
    pub fn new() -> Self {
        Self {
            header: String::new(),
            axiom_blocks: Vec::new(),
        }
    }

    pub fn set_header(&mut self, owl_blocks: &[String]) -> Result<()> {
        if owl_blocks.is_empty() {
            anyhow::bail!("No OWL blocks found in ontology definition");
        }

        self.header = owl_blocks.join("\n\n");
        Ok(())
    }

    pub fn add_owl_blocks(&mut self, owl_blocks: &[String]) -> Result<()> {
        for block in owl_blocks {
            if !block.trim().is_empty() {
                self.axiom_blocks.push(block.clone());
            }
        }
        Ok(())
    }

    pub fn add_axioms(&mut self, axioms: &[String]) -> Result<()> {
        for axiom in axioms {
            if !axiom.trim().is_empty() {
                self.axiom_blocks.push(axiom.clone());
            }
        }
        Ok(())
    }

    pub fn to_string(&self) -> String {
        let mut result = String::new();

        let header = self.header.trim();

        if header.ends_with(')') {
            let header_without_close = &header[..header.len() - 1];
            result.push_str(header_without_close);
            result.push('\n');
        } else {
            result.push_str(header);
            result.push('\n');
        }

        for block in &self.axiom_blocks {
            result.push('\n');

            for line in block.lines() {
                if !line.trim().is_empty() {
                    result.push_str("  ");
                    result.push_str(line);
                    result.push('\n');
                }
            }
        }

        result.push_str(")\n");

        result
    }

    pub fn validate(&self) -> Result<()> {
        use horned_owl::io::ofn::reader::read as read_ofn;
        use horned_owl::ontology::set::SetOntology;
        use std::io::Cursor;
        use std::sync::Arc;

        let ontology_text = self.to_string();
        let cursor = Cursor::new(ontology_text.as_bytes());

        match read_ofn::<Arc<str>, SetOntology<Arc<str>>, _>(cursor, Default::default()) {
            Ok((_ontology, _prefixes)) => {
                log::info!("  Parsed successfully");
                log::info!("  OWL Functional Syntax is valid");

                log::info!(
                    "  For full reasoning/consistency checking, use a DL reasoner like whelk-rs"
                );

                Ok(())
            }
            Err(e) => {
                anyhow::bail!("Failed to parse ontology: {:?}", e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_assembly() {
        let mut assembler = OntologyAssembler::new();

        let header = vec![r#"Prefix(mv:=<https://metaverse-ontology.org/>)
Ontology(<https://metaverse-ontology.org/>
  Declaration(Class(mv:Entity))
)"#
        .to_string()];

        assembler.set_header(&header).unwrap();

        let axioms = vec!["Declaration(Class(mv:Avatar))".to_string()];

        assembler.add_owl_blocks(&axioms).unwrap();

        let result = assembler.to_string();
        assert!(result.contains("Declaration(Class(mv:Entity))"));
        assert!(result.contains("Declaration(Class(mv:Avatar))"));
    }
}
