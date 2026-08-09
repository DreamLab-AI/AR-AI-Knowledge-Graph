//! Locality-Sensitive Hashing (LSH) for approximate nearest-neighbor similarity search.
//!
//! Replaces O(n^2) pairwise similarity with sub-quadratic candidate generation using
//! banded MinHash signatures. Each node's features (label, node_type, metadata keys)
//! are shingled into character n-grams, hashed with multiple independent hash families,
//! and organized into bands. Two nodes that collide in at least one band become
//! candidates for full similarity computation.
//!
//! ## Complexity
//!
//! - Build: O(n * signature_length)
//! - Query: O(n) expected with high selectivity from banding
//! - Full pipeline: O(n * k) where k is average candidates per node (k << n)

use std::collections::{HashMap, HashSet};

use visionclaw_domain::models::metadata::MetadataStore;
use visionclaw_domain::models::node::Node;

/// Configuration for the LSH index.
#[derive(Debug, Clone)]
pub struct LshConfig {
    /// Number of bands for the banding technique. More bands = higher recall, lower precision.
    pub num_bands: usize,
    /// Number of rows (hash functions) per band. More rows = higher precision, lower recall.
    pub rows_per_band: usize,
    /// Character n-gram size for shingling node features.
    pub shingle_size: usize,
}

impl Default for LshConfig {
    fn default() -> Self {
        Self {
            num_bands: 20,
            rows_per_band: 5,
            shingle_size: 3,
        }
    }
}

impl LshConfig {
    /// Total signature length = num_bands * rows_per_band.
    pub fn signature_length(&self) -> usize {
        self.num_bands * self.rows_per_band
    }
}

/// A single hash function from the family: h(x) = (a * x + b) mod p mod range.
/// Uses large prime for universal hashing.
#[derive(Debug, Clone)]
struct HashFunction {
    a: u64,
    b: u64,
}

impl HashFunction {
    fn new(seed: u64) -> Self {
        // Derive two pseudo-independent coefficients from the seed using bit mixing.
        let a = Self::mix(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
        let b = Self::mix(seed.wrapping_mul(0x6C62272E07BB0142).wrapping_add(7));
        Self { a, b }
    }

    /// Applies the hash function to a shingle hash value.
    fn apply(&self, shingle_hash: u64) -> u64 {
        // Mersenne-prime-style universal hash: ((a * x + b) mod p)
        // Using wrapping arithmetic as the "mod 2^64" universe, then mixing.
        let v = self.a.wrapping_mul(shingle_hash).wrapping_add(self.b);
        Self::mix(v)
    }

    /// Bit-mixing finalizer (similar to splitmix64) for good distribution.
    fn mix(mut x: u64) -> u64 {
        x ^= x >> 30;
        x = x.wrapping_mul(0xBF58476D1CE4E5B9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94D049BB133111EB);
        x ^= x >> 31;
        x
    }
}

/// Locality-Sensitive Hashing index for approximate nearest-neighbor candidate generation.
///
/// Uses banded MinHash signatures to identify candidate pairs that are likely similar,
/// avoiding the need for exhaustive O(n^2) pairwise comparison.
pub struct LshIndex {
    config: LshConfig,
    /// One hash table per band. Key = band hash, Value = list of node IDs.
    tables: Vec<HashMap<u64, Vec<u32>>>,
    /// Hash function family: one per row in the signature.
    hash_functions: Vec<HashFunction>,
    /// Stored signatures for all indexed nodes, keyed by node ID.
    signatures: HashMap<u32, Vec<u64>>,
}

impl LshIndex {
    /// Create a new LSH index with the given configuration.
    pub fn new(config: LshConfig) -> Self {
        let sig_len = config.signature_length();
        let hash_functions: Vec<HashFunction> =
            (0..sig_len).map(|i| HashFunction::new(i as u64)).collect();
        let tables = vec![HashMap::new(); config.num_bands];

        Self {
            config,
            tables,
            hash_functions,
            signatures: HashMap::new(),
        }
    }

    /// Create a new LSH index with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(LshConfig::default())
    }

    /// Insert a node into the index using its pre-computed MinHash signature.
    pub fn insert(&mut self, node_id: u32, signature: &[u64]) {
        debug_assert_eq!(
            signature.len(),
            self.config.signature_length(),
            "Signature length mismatch"
        );

        self.signatures.insert(node_id, signature.to_vec());

        for band_idx in 0..self.config.num_bands {
            let band_hash = self.compute_band_hash(signature, band_idx);
            self.tables[band_idx]
                .entry(band_hash)
                .or_default()
                .push(node_id);
        }
    }

    /// Query the index for candidate nodes similar to the given signature.
    /// Returns all node IDs that share at least one band hash with the query.
    pub fn query_candidates(&self, signature: &[u64]) -> HashSet<u32> {
        let mut candidates = HashSet::new();

        for band_idx in 0..self.config.num_bands {
            let band_hash = self.compute_band_hash(signature, band_idx);
            if let Some(bucket) = self.tables[band_idx].get(&band_hash) {
                for &node_id in bucket {
                    candidates.insert(node_id);
                }
            }
        }

        candidates
    }

    /// Build an LSH index from a slice of nodes, extracting features from labels,
    /// node types, and metadata store topic keys.
    pub fn build_from_nodes(nodes: &[Node], metadata: Option<&MetadataStore>) -> Self {
        let config = LshConfig::default();
        let mut index = Self::new(config);

        for node in nodes {
            let features = Self::extract_node_features(node, metadata);
            let signature = index.compute_signature(&features);
            index.insert(node.id, &signature);
        }

        index
    }

    /// Build an LSH index from nodes with a custom configuration.
    pub fn build_from_nodes_with_config(
        nodes: &[Node],
        metadata: Option<&MetadataStore>,
        config: LshConfig,
    ) -> Self {
        let mut index = Self::new(config);

        for node in nodes {
            let features = Self::extract_node_features(node, metadata);
            let signature = index.compute_signature(&features);
            index.insert(node.id, &signature);
        }

        index
    }

    /// Retrieve all candidate pairs from the index. Each pair (a, b) satisfies a < b
    /// and the two nodes share at least one band hash. This is the primary entry point
    /// for replacing the O(n^2) pairwise loop.
    pub fn all_candidate_pairs(&self) -> HashSet<(u32, u32)> {
        let mut pairs = HashSet::new();

        for table in &self.tables {
            for bucket in table.values() {
                if bucket.len() < 2 {
                    continue;
                }
                for i in 0..bucket.len() {
                    for j in (i + 1)..bucket.len() {
                        let a = bucket[i].min(bucket[j]);
                        let b = bucket[i].max(bucket[j]);
                        pairs.insert((a, b));
                    }
                }
            }
        }

        pairs
    }

    /// Compute the MinHash signature for a set of feature strings.
    pub fn compute_signature(&self, features: &[String]) -> Vec<u64> {
        let shingles = self.shingle_features(features);
        let sig_len = self.config.signature_length();
        let mut signature = vec![u64::MAX; sig_len];

        for shingle_hash in &shingles {
            for (i, hf) in self.hash_functions.iter().enumerate() {
                let h = hf.apply(*shingle_hash);
                if h < signature[i] {
                    signature[i] = h;
                }
            }
        }

        signature
    }

    /// Get the stored signature for a node, if it exists.
    pub fn get_signature(&self, node_id: u32) -> Option<&[u64]> {
        self.signatures.get(&node_id).map(|v| v.as_slice())
    }

    /// Estimate Jaccard similarity between two signatures using MinHash.
    /// Returns a value in [0.0, 1.0].
    pub fn estimate_similarity(sig_a: &[u64], sig_b: &[u64]) -> f32 {
        if sig_a.len() != sig_b.len() || sig_a.is_empty() {
            return 0.0;
        }
        let matching = sig_a
            .iter()
            .zip(sig_b.iter())
            .filter(|(a, b)| a == b)
            .count();
        matching as f32 / sig_a.len() as f32
    }

    /// Number of nodes currently in the index.
    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.signatures.is_empty()
    }

    // --- Private helpers ---

    /// Compute the hash for a specific band from a signature.
    fn compute_band_hash(&self, signature: &[u64], band_idx: usize) -> u64 {
        let start = band_idx * self.config.rows_per_band;
        let end = start + self.config.rows_per_band;
        let band_slice = &signature[start..end];

        // Combine band rows with FNV-1a style mixing.
        let mut hash: u64 = 0xCBF29CE484222325;
        for &val in band_slice {
            hash ^= val;
            hash = hash.wrapping_mul(0x100000001B3);
        }
        hash
    }

    /// Shingle feature strings into character n-gram hashes.
    fn shingle_features(&self, features: &[String]) -> Vec<u64> {
        let mut shingles = Vec::new();
        let n = self.config.shingle_size;

        for feature in features {
            let chars: Vec<char> = feature.chars().collect();
            if chars.len() < n {
                // Hash the entire short feature as a single shingle.
                shingles.push(Self::hash_str(feature));
            } else {
                for window in chars.windows(n) {
                    let s: String = window.iter().collect();
                    shingles.push(Self::hash_str(&s));
                }
            }
        }

        // Deduplicate shingle hashes for the MinHash to operate on a set.
        shingles.sort_unstable();
        shingles.dedup();
        shingles
    }

    /// Extract textual features from a node for shingling. Combines label, node_type,
    /// metadata keys, and topic keys from the metadata store.
    fn extract_node_features(node: &Node, metadata: Option<&MetadataStore>) -> Vec<String> {
        let mut features = Vec::new();

        // Label contributes heavily to identity.
        if !node.label.is_empty() {
            let normalized = node.label.to_lowercase();
            features.push(format!("label:{}", normalized));
            // Also add individual words for partial matching.
            for word in normalized.split_whitespace() {
                if word.len() >= 2 {
                    features.push(format!("word:{}", word));
                }
            }
        }

        // Node type as a feature.
        if let Some(ref nt) = node.node_type {
            features.push(format!("type:{}", nt.to_lowercase()));
        }

        // Group membership.
        if let Some(ref group) = node.group {
            features.push(format!("group:{}", group.to_lowercase()));
        }

        // OWL class IRI.
        if let Some(ref iri) = node.owl_class_iri {
            features.push(format!("owl:{}", iri.to_lowercase()));
        }

        // Metadata map keys and values.
        for (k, v) in &node.metadata {
            features.push(format!("meta:{}={}", k.to_lowercase(), v.to_lowercase()));
        }

        // Topic keys from the metadata store (high-value semantic signal).
        if let Some(store) = metadata {
            if let Some(meta) = store.get(&node.metadata_id) {
                for topic in meta.topic_counts.keys() {
                    features.push(format!("topic:{}", topic.to_lowercase()));
                }
                // Source domain is a strong clustering signal.
                if let Some(ref domain) = meta.source_domain {
                    features.push(format!("domain:{}", domain.to_lowercase()));
                }
                // OWL class from metadata.
                if let Some(ref owl_class) = meta.owl_class {
                    features.push(format!("owl_meta:{}", owl_class.to_lowercase()));
                }
                // Belongs-to-domain tags.
                for domain in &meta.belongs_to_domain {
                    features.push(format!("btd:{}", domain.to_lowercase()));
                }
            }
        }

        features
    }

    /// FNV-1a hash of a string, producing a u64.
    fn hash_str(s: &str) -> u64 {
        let mut hash: u64 = 0xCBF29CE484222325;
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001B3);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use visionclaw_domain::models::metadata::Metadata;
    use visionclaw_domain::models::node::Node;

    fn make_node(id: u32, label: &str, node_type: Option<&str>) -> Node {
        let mut node = Node::new_with_id(label.to_string(), Some(id));
        node.label = label.to_string();
        node.node_type = node_type.map(|s| s.to_string());
        node
    }

    #[test]
    fn test_lsh_index_creation() {
        let index = LshIndex::with_defaults();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_signature_length() {
        let config = LshConfig {
            num_bands: 10,
            rows_per_band: 4,
            shingle_size: 3,
        };
        assert_eq!(config.signature_length(), 40);
    }

    #[test]
    fn test_identical_nodes_are_candidates() {
        let nodes = vec![
            make_node(1, "machine learning algorithms", Some("concept")),
            make_node(2, "machine learning algorithms", Some("concept")),
            make_node(3, "cooking recipes for beginners", Some("tutorial")),
        ];

        let index = LshIndex::build_from_nodes(&nodes, None);
        assert_eq!(index.len(), 3);

        let pairs = index.all_candidate_pairs();
        // Identical label+type nodes must be candidates.
        assert!(
            pairs.contains(&(1, 2)),
            "Identical nodes should be candidates"
        );
    }

    #[test]
    fn test_similar_nodes_are_candidates() {
        let nodes = vec![
            make_node(1, "deep learning neural networks", Some("concept")),
            make_node(2, "deep learning architectures", Some("concept")),
            make_node(3, "italian pasta cooking recipes", Some("recipe")),
        ];

        let index = LshIndex::build_from_nodes(&nodes, None);
        let pairs = index.all_candidate_pairs();

        // The two deep learning nodes share many shingles; they should appear as candidates.
        assert!(
            pairs.contains(&(1, 2)),
            "Similar nodes should be candidates, got: {:?}",
            pairs
        );
    }

    #[test]
    fn test_dissimilar_nodes_less_likely_candidates() {
        // With enough separation in features, dissimilar nodes should not collide.
        // This is probabilistic, so we use very different strings.
        let nodes = vec![
            make_node(
                1,
                "quantum chromodynamics particle physics",
                Some("science"),
            ),
            make_node(2, "baroque harpsichord musical composition", Some("music")),
        ];

        let index = LshIndex::build_from_nodes(&nodes, None);
        let pairs = index.all_candidate_pairs();

        // These are very different; the probability of collision should be low.
        // This test documents expected behavior but may rarely fail due to hash collisions.
        // If it fails, increase num_bands or rows_per_band.
        assert!(
            !pairs.contains(&(1, 2)),
            "Very dissimilar nodes should rarely be candidates"
        );
    }

    #[test]
    fn test_query_candidates_returns_self_and_similar() {
        let nodes = vec![
            make_node(1, "reinforcement learning agent", Some("concept")),
            make_node(2, "reinforcement learning policy", Some("concept")),
            make_node(3, "underwater basket weaving", Some("hobby")),
        ];

        let index = LshIndex::build_from_nodes(&nodes, None);

        let sig1 = index.get_signature(1).unwrap();
        let candidates = index.query_candidates(sig1);

        assert!(candidates.contains(&1), "Self should be in candidates");
        assert!(
            candidates.contains(&2),
            "Similar node should be in candidates"
        );
    }

    #[test]
    fn test_estimate_similarity_identical() {
        let sig = vec![1u64, 2, 3, 4, 5];
        let sim = LshIndex::estimate_similarity(&sig, &sig);
        assert!((sim - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_estimate_similarity_disjoint() {
        let sig_a = vec![1u64, 2, 3, 4, 5];
        let sig_b = vec![6u64, 7, 8, 9, 10];
        let sim = LshIndex::estimate_similarity(&sig_a, &sig_b);
        assert!((sim - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_build_with_metadata() {
        let nodes = vec![
            make_node(1, "AI Overview", None),
            make_node(2, "Machine Learning", None),
        ];

        let mut store: MetadataStore = HashMap::new();

        let mut ai_topics = HashMap::new();
        ai_topics.insert("artificial_intelligence".to_string(), 10);
        ai_topics.insert("technology".to_string(), 5);

        let mut ml_topics = HashMap::new();
        ml_topics.insert("machine_learning".to_string(), 15);
        ml_topics.insert("artificial_intelligence".to_string(), 8);

        store.insert(
            "AI Overview".to_string(),
            Metadata {
                file_name: "ai_overview.md".to_string(),
                topic_counts: ai_topics,
                ..Default::default()
            },
        );

        store.insert(
            "Machine Learning".to_string(),
            Metadata {
                file_name: "machine_learning.md".to_string(),
                topic_counts: ml_topics,
                ..Default::default()
            },
        );

        // Use a high-recall config (many bands, 1 row each) so that any single hash
        // collision makes the pair a candidate.  This avoids flaky results from the
        // inherently probabilistic MinHash when the Jaccard similarity is moderate.
        let high_recall_config = LshConfig {
            num_bands: 100,
            rows_per_band: 1,
            shingle_size: 3,
        };
        let index =
            LshIndex::build_from_nodes_with_config(&nodes, Some(&store), high_recall_config);
        assert_eq!(index.len(), 2);

        // Both share the "artificial_intelligence" topic, so they should be candidates.
        let pairs = index.all_candidate_pairs();
        assert!(
            pairs.contains(&(1, 2)),
            "Nodes sharing topics should be candidates"
        );
    }

    #[test]
    fn test_custom_config() {
        let config = LshConfig {
            num_bands: 5,
            rows_per_band: 3,
            shingle_size: 2,
        };

        let nodes = vec![
            make_node(1, "test node alpha", None),
            make_node(2, "test node beta", None),
        ];

        let index = LshIndex::build_from_nodes_with_config(&nodes, None, config);
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn test_empty_label_handling() {
        let nodes = vec![make_node(1, "", None), make_node(2, "", None)];

        let index = LshIndex::build_from_nodes(&nodes, None);
        assert_eq!(index.len(), 2);
        // Should not panic on empty features.
    }

    #[test]
    fn test_scale_candidate_count() {
        // Verify that LSH produces far fewer than n*(n-1)/2 pairs for dissimilar nodes.
        let mut nodes = Vec::new();
        for i in 0..100 {
            nodes.push(make_node(
                i,
                &format!("unique_topic_{}_with_long_label_{}", i, i * 31),
                Some(&format!("type_{}", i % 10)),
            ));
        }

        let index = LshIndex::build_from_nodes(&nodes, None);
        let pairs = index.all_candidate_pairs();
        let exhaustive_pairs = 100 * 99 / 2;

        // LSH should produce significantly fewer candidates than exhaustive.
        assert!(
            pairs.len() < exhaustive_pairs,
            "LSH should reduce pair count: {} vs {} exhaustive",
            pairs.len(),
            exhaustive_pairs
        );
    }
}
