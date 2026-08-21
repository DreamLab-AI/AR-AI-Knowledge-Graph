//! ADR-031 D7 — Analytics correctness-as-contract suite: GPU-gated companions.
//!
//! The CPU-reference known-answer tests (two_clique/pagerank/dbscan/lof), the
//! property-based invariants, the NFR checks, and the writer invariants moved to
//! the CUDA-free `visionclaw-analytics-oracle` crate
//! (`crates/visionclaw-analytics-oracle/tests/analytics_correctness.rs`) so the
//! ubuntu `CPU_CRATES` CI job (`cargo test`) actually runs them as a merge gate —
//! this root crate links libcuda and is excluded from that job, so the suite
//! previously gated nothing.
//!
//! What remains here are the tests that genuinely need a real CUDA device +
//! compiled PTX. They are `#[ignore]` and run on the developer's GPU host via
//! `cargo test -- --ignored`. A host without a CUDA device or compiled PTX skips
//! cleanly (lacking a GPU is not a correctness failure); only a real bad result
//! fails. The CPU oracle they are asserted against is the crate above.

#[path = "analytics_fixtures.rs"]
mod fx;

#[allow(unused_imports)]
use fx::*;

// ===========================================================================
// GPU-GATED KNOWN-ANSWER + ORACLE TESTS (real CUDA device + compiled PTX)
// ===========================================================================

mod louvain {
    use super::*;

    /// GPU Louvain on the canonical fixture must clear the gate AND converge.
    /// End-to-end through the real GPU path: load the compiled PTX, build the
    /// undirected CSR, run `run_louvain_community_detection`, and assert both the
    /// kernel-reported modularity (the value the D1 gate consumes) and an
    /// independent CPU recomputation of the returned labels clear Q >= 0.3.
    ///
    /// Runs on the GPU CI runner via `cargo test -- --ignored`. A host without a
    /// CUDA device or compiled PTX skips cleanly (it is not a correctness
    /// failure to lack a GPU); only a real low-modularity result fails.
    #[test]
    #[ignore = "needs GPU: real CUDA device + compiled PTX (run with --ignored on GPU CI)"]
    fn gpu_louvain_clears_gate_on_canonical() {
        use visionclaw_gpu::ptx_loader::{load_ptx_module_sync, PTXModule};
        use visionclaw_server::utils::unified_gpu_compute::UnifiedGPUCompute;

        let g = canonical_live_scale();
        let n = g.n;

        // Symmetric (both-direction) unit-weight CSR — the layout the live graph
        // feeds the GPU and that run_louvain's degree kernel expects.
        let mut adj: Vec<Vec<i32>> = vec![Vec::new(); n];
        for &(u, v) in &g.edges {
            adj[u as usize].push(v as i32);
            adj[v as usize].push(u as i32);
        }
        let mut offsets: Vec<i32> = Vec::with_capacity(n + 1);
        let mut indices: Vec<i32> = Vec::new();
        offsets.push(0);
        for a in &adj {
            indices.extend_from_slice(a);
            offsets.push(indices.len() as i32);
        }
        let weights = vec![1.0f32; indices.len()];
        let num_edges = indices.len(); // directed edge count (2× undirected)

        let unified = match load_ptx_module_sync(PTXModule::VisionflowUnified) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "SKIP gpu_louvain_clears_gate_on_canonical: unified PTX unavailable: {e}"
                );
                return;
            }
        };
        let clustering = match load_ptx_module_sync(PTXModule::GpuClusteringKernels) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "SKIP gpu_louvain_clears_gate_on_canonical: clustering PTX unavailable: {e}"
                );
                return;
            }
        };

        let mut gpu = match UnifiedGPUCompute::new_with_modules(
            n,
            num_edges,
            &unified,
            Some(&clustering),
            None,
        ) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("SKIP gpu_louvain_clears_gate_on_canonical: no CUDA device: {e}");
                return;
            }
        };

        gpu.upload_edges_csr(&offsets, &indices, &weights)
            .expect("upload_edges_csr");

        let (labels, num_comm, gpu_modularity, _iters, _sizes, converged) = gpu
            .run_louvain_community_detection(100, 1.0, 42)
            .expect("run_louvain_community_detection");

        assert!(
            converged,
            "Louvain must converge on the canonical fixture (16 planted communities)"
        );
        assert!(
            num_comm >= 2,
            "must resolve multiple communities, got {num_comm}"
        );
        assert!(
            gpu_modularity >= 0.3,
            "kernel-reported modularity {gpu_modularity:.4} must clear the D1 gate (>= 0.3)"
        );

        // Independent CPU recomputation of the GPU labels — catches a kernel that
        // reports a good Q but writes inconsistent labels.
        let labels_u32: Vec<u32> = labels.iter().take(n).map(|&l| l.max(0) as u32).collect();
        assert_eq!(labels_u32.len(), n, "GPU returned one label per node");
        let q_cpu = modularity(&g, &labels_u32);
        assert!(
            q_cpu >= 0.3,
            "CPU recomputation of GPU community labels {q_cpu:.4} must clear the gate"
        );
    }
}

mod pagerank_tests {
    #[test]
    #[ignore = "needs GPU: correct global-dangling pagerank.cu kernel via FFI (D8)"]
    fn gpu_pagerank_matches_cpu_reference() {
        // Intended: switch FFI to pagerank.cu:186-261, read centrality back,
        // assert within tolerance of pagerank(&g, 0.85, 100) and sums to 1.0.
        panic!("bind to corrected PageRank FFI once the per-block kernel is removed");
    }
}

mod dbscan_tests {
    #[test]
    #[ignore = "needs GPU: gpu_clustering_kernels DBSCAN border-assignment fix"]
    fn gpu_dbscan_matches_cpu_reference() {
        panic!("bind to GPU DBSCAN once border-assignment lands");
    }
}

mod lof_tests {
    /// The LOF kernel itself is FIXED: `compute_lof_kernel` computes the real
    /// Breunig ratio ( mean(lrd(neighbours)) / lrd(self) ) at
    /// `crates/visionclaw-gpu/src/cuda_sources/gpu_clustering_kernels.cu:489-493`,
    /// replacing the earlier `1/local_density` placeholder. What is still
    /// outstanding is only the GPU test-hook harness that gathers the point set,
    /// invokes the kernel through the FFI, reads the scores back, and asserts them
    /// within tolerance of the CPU oracle `lof(&pts, k)` — the same blocker as the
    /// sibling `gpu_pagerank`/`gpu_dbscan` stubs. Runs on the GPU host via
    /// `--ignored`; the CPU-reference LOF assertions already gate in
    /// `visionclaw-analytics-oracle`.
    #[test]
    #[ignore = "needs GPU: bind the (already-fixed) LOF ratio kernel to the oracle harness"]
    fn gpu_lof_matches_cpu_reference() {
        panic!(
            "LOF kernel is fixed (real ratio at gpu_clustering_kernels.cu:489-493); \
             bind it to the GPU oracle harness to assert against lof(&pts, k)"
        );
    }
}

mod oracle {
    #[test]
    #[ignore = "needs GPU: cross-check every kernel against its CPU twin"]
    fn gpu_cpu_oracle_full_matrix() {
        // Intended harness once GPU hooks exist:
        //   for g in [triangle(), star(6), linear_chain(10), two_clique()] {
        //       assert_vec_close(&pagerank(&g,0.85,100), &gpu_pagerank(&g), 1e-3, g.name);
        //   }
        panic!("bind GPU kernels to oracle harness once test hooks are exposed");
    }
}
