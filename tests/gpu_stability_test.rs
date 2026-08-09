//! GPU Stability Gate Tests
//!
//! Tests for GPU physics stability detection, including kinetic energy thresholds
//! and per-node stability optimization.
//!
//! These tests require CUDA GPU hardware and access to the compiled PTX kernels.
//! They are marked `#[ignore]` and must be run explicitly with `--ignored`.
//!
//! NOTE: The UnifiedGPUCompute API has evolved (upload_node_data / upload_edge_data
//! were replaced with upload_edges_csr and direct buffer manipulation). These tests
//! document the intended behavior but need API updates before they can pass.

#[cfg(test)]
mod gpu_stability_tests {
    #[allow(unused_imports)]
    use std::fs;
    #[allow(unused_imports)]
    use visionclaw_server::models::simulation_params::SimParams;
    use visionclaw_server::utils::unified_gpu_compute::UnifiedGPUCompute;

    #[test]
    #[ignore] // Requires CUDA GPU and updated UnifiedGPUCompute API
    fn test_stability_gate_activation() {
        // Load PTX content
        let ptx_path = concat!(env!("OUT_DIR"), "/visionclaw_unified.ptx");
        let ptx_content = fs::read_to_string(ptx_path).expect("Failed to read PTX file");

        // Create GPU compute with simple graph
        let num_nodes = 100;
        let num_edges = 200;

        // NOTE: UnifiedGPUCompute::new() API has changed — this needs updating
        // to use new_with_modules() or the current constructor signature.
        // The test documents the intended stability-gate behavior:
        // after enough iterations with high damping, system kinetic energy
        // should drop below stability_threshold * num_nodes.
        let _gpu_compute =
            UnifiedGPUCompute::new_with_modules(num_nodes, num_edges, &ptx_content, None, None)
                .expect("Failed to create GPU compute");

        // TODO: Update to current API for uploading node/edge data
        // and running the physics simulation loop.

        // The stability gate should activate when:
        //   system_kinetic_energy < stability_threshold * num_nodes
        // after sufficient iterations with damping = 0.95.
    }

    #[test]
    #[ignore] // Requires CUDA GPU and updated UnifiedGPUCompute API
    fn test_per_node_stability_optimization() {
        // Load PTX content
        let ptx_path = concat!(env!("OUT_DIR"), "/visionclaw_unified.ptx");
        let ptx_content = fs::read_to_string(ptx_path).expect("Failed to read PTX file");

        let num_nodes = 1000;
        let num_edges = 0;

        let _gpu_compute =
            UnifiedGPUCompute::new_with_modules(num_nodes, num_edges, &ptx_content, None, None)
                .expect("Failed to create GPU compute");

        // TODO: Update to current API for uploading positions/velocities.
        //
        // Test intent: with half the nodes stationary (vel=0) and half moving
        // (vel=1.0), the active_node_count should be approximately num_nodes/2
        // (within 40-60% range).
    }
}
