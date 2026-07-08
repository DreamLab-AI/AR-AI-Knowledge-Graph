//! Precedent registry — tracks approved enrichment scopes for auto-approval.
//!
//! When a broker marks a decision as `Precedent { scope }`, the scope is
//! registered with a hit count. When `check` is called for a matching scope,
//! the registry returns `true` if the hit count meets the threshold — meaning
//! the case can be auto-approved without human review.

use std::collections::HashMap;

const DEFAULT_THRESHOLD: u32 = 3;

pub struct PrecedentRegistry {
    scopes: HashMap<String, u32>,
    threshold: u32,
}

impl Default for PrecedentRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_THRESHOLD)
    }
}

impl PrecedentRegistry {
    pub fn new(threshold: u32) -> Self {
        Self {
            scopes: HashMap::new(),
            threshold,
        }
    }

    /// Register a scope as a precedent (increments hit count).
    pub fn register(&mut self, scope: &str) {
        *self.scopes.entry(scope.to_string()).or_insert(0) += 1;
    }

    /// Check if a scope has enough precedents for auto-approval.
    pub fn qualifies(&self, scope: &str) -> bool {
        self.scopes
            .get(scope)
            .map(|&count| count >= self.threshold)
            .unwrap_or(false)
    }

    /// Build a precedent scope key from enrichment metadata.
    /// Format: `{enrichment_type}:{entity_class}` where entity_class is
    /// the first two segments of the URN (e.g. `concept:bc` from
    /// `urn:visionclaw:concept:bc:smart-contract`).
    pub fn scope_from_metadata(enrichment_type: &str, entity_urn: &str) -> String {
        let class = entity_urn
            .strip_prefix("urn:visionclaw:")
            .and_then(|rest| {
                let parts: Vec<&str> = rest.splitn(3, ':').collect();
                if parts.len() >= 2 {
                    Some(format!("{}:{}", parts[0], parts[1]))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string());
        format!("{}:{}", enrichment_type, class)
    }

    pub fn scope_count(&self, scope: &str) -> u32 {
        self.scopes.get(scope).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_qualify() {
        let mut reg = PrecedentRegistry::new(2);
        let scope = "embedding_update:concept:bc";
        assert!(!reg.qualifies(scope));
        reg.register(scope);
        assert!(!reg.qualifies(scope));
        reg.register(scope);
        assert!(reg.qualifies(scope));
    }

    #[test]
    fn scope_from_metadata_extracts_class() {
        assert_eq!(
            PrecedentRegistry::scope_from_metadata(
                "embedding_update",
                "urn:visionclaw:concept:bc:smart-contract"
            ),
            "embedding_update:concept:bc"
        );
    }

    #[test]
    fn scope_from_metadata_handles_missing_prefix() {
        assert_eq!(
            PrecedentRegistry::scope_from_metadata("gap_detection", "some-random-id"),
            "gap_detection:unknown"
        );
    }
}
