// src/ports/semantic_analyzer.rs
//! Semantic Analyzer Port
//!
//! Defines the interface for semantic graph analysis operations.
//! Abstracts GPU-accelerated algorithms, CPU fallbacks, or external services.

use async_trait::async_trait;
use std::collections::HashMap;

use visionclaw_domain::models::graph::GraphData;

pub type Result<T> = std::result::Result<T, SemanticAnalyzerError>;

#[derive(Debug, thiserror::Error)]
pub enum SemanticAnalyzerError {
    #[error("Analysis error: {0}")]
    AnalysisError(String),

    #[error("Invalid graph: {0}")]
    InvalidGraph(String),

    #[error("Algorithm not supported: {0}")]
    UnsupportedAlgorithm(String),
}

#[derive(Debug, Clone)]
pub struct SSSPResult {
    pub source: u32,
    pub distances: HashMap<u32, f32>,
    pub predecessors: HashMap<u32, u32>,
}

#[derive(Debug, Clone)]
pub struct ClusteringResult {
    pub clusters: HashMap<u32, usize>,
    pub cluster_count: usize,
    pub modularity: f32,
}

#[derive(Debug, Clone)]
pub struct CommunityResult {
    pub communities: HashMap<u32, usize>,
    pub community_count: usize,
    pub modularity: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum ClusterAlgorithm {
    Louvain,
    LabelPropagation,
    ConnectedComponents,
}

#[async_trait]
pub trait SemanticAnalyzer: Send + Sync {
    async fn run_sssp(&self, graph: &GraphData, source: u32) -> Result<SSSPResult>;

    async fn run_clustering(
        &self,
        graph: &GraphData,
        algorithm: ClusterAlgorithm,
    ) -> Result<ClusteringResult>;

    async fn detect_communities(&self, graph: &GraphData) -> Result<CommunityResult>;

    async fn get_shortest_path(
        &self,
        graph: &GraphData,
        source: u32,
        target: u32,
    ) -> Result<Vec<u32>>;

    async fn invalidate_cache(&self) -> Result<()>;
}
