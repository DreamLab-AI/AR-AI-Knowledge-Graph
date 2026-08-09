//! Physics simulation execution pipeline (force computation, integration, stability).

use super::construction::UnifiedGPUCompute;
use super::types::{int3, thrust_sort_key_value, AABB};
use crate::models::simulation_params::{SimParams, ToSimParams};
use anyhow::{anyhow, Result};
use cust::context::Context;
use cust::launch;
use cust::memory::{CopyDestination, DeviceBuffer, DevicePointer};
use log::{debug, info, warn};
use std::ffi::CStr;

fn safe_copy_to_device<T: cust::memory::DeviceCopy>(
    dest: &mut DeviceBuffer<T>,
    src: &[T],
    label: &str,
) -> Result<()> {
    if dest.len() != src.len() {
        return Err(anyhow!(
            "copy_from size mismatch in {}: device buffer has {} elements, host slice has {} elements",
            label, dest.len(), src.len()
        ));
    }
    dest.copy_from(src)
        .map_err(|e| anyhow!("copy_from failed in {}: {}", label, e))
}

fn safe_copy_from_device<T: cust::memory::DeviceCopy>(
    src: &DeviceBuffer<T>,
    dest: &mut [T],
    label: &str,
) -> Result<()> {
    if src.len() != dest.len() {
        return Err(anyhow!(
            "copy_to size mismatch in {}: device buffer has {} elements, host slice has {} elements",
            label, src.len(), dest.len()
        ));
    }
    src.copy_to(dest)
        .map_err(|e| anyhow!("copy_to failed in {}: {}", label, e))
}

impl UnifiedGPUCompute {
    /// Default block size for kernel launches.  Ideally this would be queried
    /// from `dynamic_grid.cu::calculate_optimal_block_size()` at init time, but
    /// there is no Rust FFI wrapper for that function yet.  This constant can be
    /// overridden via the `VISIONCLAW_BLOCK_SIZE` environment variable for
    /// tuning without recompilation.
    // TODO: Wire to dynamic_grid.cu::calculate_optimal_block_size() via FFI
    //       and cache the result in UnifiedGPUCompute at construction time.
    const DEFAULT_BLOCK_SIZE: u32 = 256;

    fn kernel_block_size() -> u32 {
        // Allow runtime override via environment variable for tuning
        static BLOCK_SIZE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        *BLOCK_SIZE.get_or_init(|| {
            std::env::var("VISIONCLAW_BLOCK_SIZE")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|&bs| bs >= 32 && bs <= 1024 && bs % 32 == 0)
                .unwrap_or(Self::DEFAULT_BLOCK_SIZE)
        })
    }

    pub fn execute(&mut self, mut params: SimParams) -> Result<()> {
        // Make CUDA context current for this thread (required when called from spawn_blocking threads)
        // Context::new() on the same device retains the primary context and makes it current
        let _thread_context = Context::new(self.device.clone())
            .map_err(|e| anyhow!("Failed to set CUDA context: {}", e))?;

        params.iteration = self.iteration;
        let block_size = Self::kernel_block_size();
        let grid_size = (self.num_nodes as u32 + block_size - 1) / block_size;

        if self.num_nodes > self.allocated_nodes {
            return Err(anyhow!("CRITICAL: num_nodes ({}) exceeds allocated_nodes ({}). This would cause buffer overflow!", self.num_nodes, self.allocated_nodes));
        }

        if self.iteration == 0 {
            info!(
                "GPU execute() iter=0 buffer audit: num_nodes={}, allocated_nodes={}, num_edges={}, allocated_edges={}, \
                 max_grid_cells={}, zero_buffer={}, cell_start={}, cell_end={}, \
                 pos_in_x={}, vel_in_x={}, force_x={}, mass={}, \
                 active_node_count={}, should_skip_physics={}, \
                 aabb_num_blocks={}, aabb_block_results={}, partial_ke={}, \
                 node_degrees={}, degree_weight={}, edge_row_offsets={}, edge_col_indices={}",
                self.num_nodes, self.allocated_nodes, self.num_edges, self.allocated_edges,
                self.max_grid_cells, self.zero_buffer.len(), self.cell_start.len(), self.cell_end.len(),
                self.pos_in_x.len(), self.vel_in_x.len(), self.force_x.len(), self.mass.len(),
                self.active_node_count.len(), self.should_skip_physics.len(),
                self.aabb_num_blocks, self.aabb_block_results.len(), self.partial_kinetic_energy.len(),
                self.node_degrees.len(), self.degree_weight.len(),
                self.edge_row_offsets.len(), self.edge_col_indices.len(),
            );
        }

        self.params = params;

        let mut c_params_global = self._module.get_global(
            CStr::from_bytes_with_nul(b"c_params\0")
                .expect("static null-terminated byte literal is always valid"),
        )?;
        c_params_global.copy_from(&[params])?;

        if self.num_nodes > 0 && params.stability_threshold > 0.0 {
            let num_blocks = (self.num_nodes + block_size as usize - 1) / block_size as usize;
            let shared_mem_size =
                block_size * (std::mem::size_of::<f32>() + std::mem::size_of::<i32>()) as u32;

            safe_copy_to_device(
                &mut self.active_node_count,
                &[0i32],
                "active_node_count reset",
            )?;
            safe_copy_to_device(
                &mut self.should_skip_physics,
                &[0i32],
                "should_skip_physics reset",
            )?;

            let ke_kernel = self
                ._module
                .get_function("calculate_kinetic_energy_kernel")?;
            // SAFETY: Kernel launch is safe because:
            // 1. All DeviceBuffer pointers (vel_in_*, mass, partial_kinetic_energy, active_node_count)
            //    are valid allocations created during UnifiedGPUCompute::new()
            // 2. num_nodes <= allocated_nodes was verified at function entry
            // 3. shared_mem_size is computed based on block_size and type sizes
            // 4. self.stream is a valid CUDA stream created in UnifiedGPUCompute::new()
            // 5. The kernel function was loaded from a valid PTX module
            unsafe {
                let stream = &self.stream;
                launch!(
                    ke_kernel<<<num_blocks as u32, block_size, shared_mem_size, stream>>>(
                        self.vel_in_x.as_device_ptr(),
                        self.vel_in_y.as_device_ptr(),
                        self.vel_in_z.as_device_ptr(),
                        self.mass.as_device_ptr(),
                        self.partial_kinetic_energy.as_device_ptr(),
                        self.active_node_count.as_device_ptr(),
                        self.num_nodes as i32,
                        params.min_velocity_threshold
                    )
                )?;
            }

            let stability_kernel = self._module.get_function("check_system_stability_kernel")?;
            let reduction_blocks = (num_blocks as u32).min(256);
            // SAFETY: Kernel launch is safe because:
            // 1. All DeviceBuffer arguments are valid allocations from UnifiedGPUCompute::new()
            // 2. reduction_blocks is bounded to max 256 (valid CUDA block size)
            // 3. Shared memory (reduction_blocks * 4) fits within GPU limits
            // 4. This reduction kernel reads from partial_kinetic_energy computed by prior kernel
            unsafe {
                let stream = &self.stream;
                launch!(
                    stability_kernel<<<1, reduction_blocks, reduction_blocks * 4, stream>>>(
                        self.partial_kinetic_energy.as_device_ptr(),
                        self.active_node_count.as_device_ptr(),
                        self.should_skip_physics.as_device_ptr(),
                        self.system_kinetic_energy.as_device_ptr(),
                        num_blocks as i32,
                        self.num_nodes as i32,
                        params.stability_threshold,
                        self.iteration
                    )
                )?;
            }

            let mut skip_physics = vec![0i32; 1];
            safe_copy_from_device(
                &self.should_skip_physics,
                &mut skip_physics,
                "should_skip_physics read",
            )?;

            if skip_physics[0] != 0 {
                self.iteration += 1;
                return Ok(());
            }
        }

        crate::utils::gpu_diagnostics::validate_kernel_launch(
            "unified_gpu_execute",
            grid_size,
            block_size,
            self.num_nodes,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

        let aabb_kernel = self._module.get_function("compute_aabb_reduction_kernel")?;
        let aabb_block_size = 256u32;
        let aabb_grid_size = self.aabb_num_blocks as u32;
        let shared_mem = 6 * aabb_block_size * std::mem::size_of::<f32>() as u32;

        // SAFETY: AABB reduction kernel launch is safe because:
        // 1. pos_in_* buffers contain valid position data from prior physics step
        // 2. aabb_block_results is sized for aabb_num_blocks * sizeof(AABB)
        // 3. shared_mem is computed as 6 floats per thread (min/max x,y,z)
        // 4. aabb_grid_size and aabb_block_size are validated during construction
        unsafe {
            let s = &self.stream;
            launch!(
                aabb_kernel<<<aabb_grid_size, aabb_block_size, shared_mem, s>>>(
                    self.pos_in_x.as_device_ptr(),
                    self.pos_in_y.as_device_ptr(),
                    self.pos_in_z.as_device_ptr(),
                    self.aabb_block_results.as_device_ptr(),
                    self.num_nodes as i32
                )
            )?;
        }

        let mut block_results = vec![AABB::default(); self.aabb_num_blocks];
        safe_copy_from_device(
            &self.aabb_block_results,
            &mut block_results,
            "aabb_block_results read",
        )?;

        let mut aabb = AABB {
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
        };
        for block_aabb in block_results.iter().take(self.aabb_num_blocks) {
            aabb.min[0] = aabb.min[0].min(block_aabb.min[0]);
            aabb.min[1] = aabb.min[1].min(block_aabb.min[1]);
            aabb.min[2] = aabb.min[2].min(block_aabb.min[2]);
            aabb.max[0] = aabb.max[0].max(block_aabb.max[0]);
            aabb.max[1] = aabb.max[1].max(block_aabb.max[1]);
            aabb.max[2] = aabb.max[2].max(block_aabb.max[2]);
        }

        let scene_volume =
            (aabb.max[0] - aabb.min[0]) * (aabb.max[1] - aabb.min[1]) * (aabb.max[2] - aabb.min[2]);
        let target_neighbors_per_cell = 8.0;
        let optimal_cells = (self.num_nodes as f32 / target_neighbors_per_cell).max(1.0);
        let optimal_cell_size = (scene_volume / optimal_cells).powf(1.0 / 3.0);

        // Choose a candidate cell size, then SANITISE it to a strictly-positive,
        // finite value. A non-positive / NaN / Inf cell size is the #81 trigger:
        // `ext / cell` becomes ±Inf or NaN, and `Inf as i32` saturates to
        // i32::MAX, producing an absurd grid that overruns the cell buffers and
        // wedges the kernel with a sticky illegal-memory-access. `grid_cell_size`
        // is user-driven (it moves with the spread/cutoff sliders), so it must
        // never be trusted blindly.
        let candidate_cell_size = if optimal_cell_size > 10.0 && optimal_cell_size < 1000.0 {
            optimal_cell_size
        } else {
            params.grid_cell_size
        };
        // Absolute floor for the cell size. The grid only needs to be at least as
        // coarse as the repulsion cutoff for the 3x3x3 neighbour search to be
        // correct; anything finer just wastes cells. Use the cutoff (when valid)
        // as a sensible lower bound and fall back to a fixed 10.0 world units.
        let cutoff_floor = if params.repulsion_cutoff.is_finite() && params.repulsion_cutoff > 0.0 {
            params.repulsion_cutoff
        } else {
            10.0
        };
        let mut auto_tuned_cell_size =
            if candidate_cell_size.is_finite() && candidate_cell_size > 0.0 {
                candidate_cell_size.max(cutoff_floor.min(10.0))
            } else {
                cutoff_floor.max(10.0)
            };

        debug!(
            "Spatial hashing: scene_volume={:.2}, optimal_cell_size={:.2}, using_size={:.2}",
            scene_volume, optimal_cell_size, auto_tuned_cell_size
        );

        aabb.min[0] -= auto_tuned_cell_size;
        aabb.max[0] += auto_tuned_cell_size;
        aabb.min[1] -= auto_tuned_cell_size;
        aabb.max[1] += auto_tuned_cell_size;
        aabb.min[2] -= auto_tuned_cell_size;
        aabb.max[2] += auto_tuned_cell_size;

        // Clamp the spatial grid to the cell-buffer capacity. When the layout
        // spreads out (e.g. a large repel_k increase), the auto-tuned cell size
        // can demand far more cells than cell_start/cell_end can hold (capped at
        // max_allowed_grid_cells). The cell hash in build_grid_kernel /
        // compute_cell_bounds_kernel is derived from grid_dims, so an
        // over-capacity grid indexes cell_start out of bounds → a sticky CUDA
        // illegal-memory-access that poisons the context and freezes physics.
        // We size the grid in CLOSED FORM against the cap (no iterative guessing
        // that can fail to converge), in i64 math to avoid i32 overflow on huge
        // spreads, and clamp every extent to a finite non-negative value first.
        let ext_x = (aabb.max[0] - aabb.min[0]).max(0.0);
        let ext_y = (aabb.max[1] - aabb.min[1]).max(0.0);
        let ext_z = (aabb.max[2] - aabb.min[2]).max(0.0);
        let (ext_x, ext_y, ext_z) = (
            if ext_x.is_finite() { ext_x } else { 0.0 },
            if ext_y.is_finite() { ext_y } else { 0.0 },
            if ext_z.is_finite() { ext_z } else { 0.0 },
        );
        let max_cells = self.max_allowed_grid_cells.max(1);
        let compute_dims = |cell: f32| -> (int3, usize) {
            // `cell` is guaranteed finite & > 0 by the callers below, so the
            // divisions are finite. Clamp each axis into [1, i32::MAX] before the
            // i32 cast so an overflow can never produce a negative dimension.
            let dim = |ext: f32| -> i32 {
                let d = (ext / cell).ceil();
                if !d.is_finite() {
                    return i32::MAX;
                }
                d.clamp(1.0, i32::MAX as f32) as i32
            };
            let dims = int3 {
                x: dim(ext_x),
                y: dim(ext_y),
                z: dim(ext_z),
            };
            let cells = (dims.x as i64 * dims.y as i64 * dims.z as i64).max(1) as usize;
            (dims, cells)
        };
        let (mut grid_dims, mut num_grid_cells) = compute_dims(auto_tuned_cell_size);
        if num_grid_cells > max_cells {
            // Closed-form minimum cell size that keeps the grid within the cap:
            // for an isotropic cell, num_cells ≈ (ext_x*ext_y*ext_z)/cell^3, so
            // cell_min = (volume / max_cells)^(1/3). +5% headroom absorbs the
            // per-axis ceil() rounding. Then take one corrective pass in case
            // rounding still pushed a single axis over.
            let volume = (ext_x as f64) * (ext_y as f64) * (ext_z as f64);
            let cell_min = (volume / max_cells as f64).cbrt() * 1.05;
            if cell_min.is_finite() && cell_min > auto_tuned_cell_size as f64 {
                auto_tuned_cell_size = cell_min as f32;
            }
            let (d, c) = compute_dims(auto_tuned_cell_size);
            grid_dims = d;
            num_grid_cells = c;
            // Final guard: if rounding STILL leaves us over the cap, grow the
            // cell size by the residual overflow ratio (bounded loop that is
            // guaranteed to terminate because cell size grows monotonically).
            for _ in 0..8 {
                if num_grid_cells <= max_cells {
                    break;
                }
                let ratio = (num_grid_cells as f64 / max_cells as f64).cbrt().max(1.0) * 1.05;
                auto_tuned_cell_size = (auto_tuned_cell_size as f64 * ratio) as f32;
                let (d, c) = compute_dims(auto_tuned_cell_size);
                grid_dims = d;
                num_grid_cells = c;
            }
            warn!(
                "Spatial grid exceeded cell-buffer cap ({} cells); enlarged cell size to {:.2} → {} cells ({}x{}x{})",
                max_cells, auto_tuned_cell_size, num_grid_cells, grid_dims.x, grid_dims.y, grid_dims.z
            );
        }

        let occupancy = self.get_grid_occupancy(num_grid_cells);
        if occupancy < 0.1 {
            warn!("Low grid occupancy detected: {:.1}% (avg {:.1} nodes/cell). Consider larger cell size.",
                  occupancy * 100.0, self.num_nodes as f32 / num_grid_cells as f32);
        } else if occupancy > 2.0 {
            warn!("High grid occupancy detected: {:.1}% (avg {:.1} nodes/cell). Consider smaller cell size.",
                  occupancy * 100.0, self.num_nodes as f32 / num_grid_cells as f32);
        }

        if num_grid_cells > self.max_grid_cells {
            self.resize_cell_buffers(num_grid_cells)?;
            debug!(
                "Grid buffer resize completed. Current grid: {}x{}x{} = {} cells",
                grid_dims.x, grid_dims.y, grid_dims.z, num_grid_cells
            );
        }

        // INVARIANT (the real #81 fix): every cell key the kernels can produce
        // must be a valid index into cell_start/cell_end. The kernels derive the
        // key from `grid_dims` in [0, grid_dims.x*y*z) = [0, num_grid_cells), so
        // `num_grid_cells` MUST be <= cell_start.len(). resize_cell_buffers caps
        // its allocation at max_allowed_grid_cells, so if the requested grid was
        // larger than that cap, the buffers are smaller than num_grid_cells and
        // the kernel would write out of bounds → sticky IMA + frozen physics.
        // Reconcile by RE-SIZING the grid down to fit the buffer that actually
        // exists, enlarging the cell size so the (now coarser) grid covers the
        // same scene with no out-of-range keys.
        let cell_capacity = self.cell_start.len().min(self.cell_end.len());
        if num_grid_cells > cell_capacity {
            // Grow the cell size until the grid fits the real buffer capacity.
            // Bounded loop; cell size grows monotonically so it always converges.
            let cap = cell_capacity.max(1);
            for _ in 0..16 {
                if num_grid_cells <= cap {
                    break;
                }
                let ratio = (num_grid_cells as f64 / cap as f64).cbrt().max(1.0) * 1.05;
                auto_tuned_cell_size = (auto_tuned_cell_size as f64 * ratio) as f32;
                let (d, c) = compute_dims(auto_tuned_cell_size);
                grid_dims = d;
                num_grid_cells = c;
            }
            // Hard backstop: if the scene is so degenerate the loop still can't
            // fit it (should be impossible after sanitisation), collapse to a
            // single cell so every node hashes to key 0 — correct, if slow.
            if num_grid_cells > cap {
                grid_dims = int3 { x: 1, y: 1, z: 1 };
                num_grid_cells = 1;
            }
            warn!(
                "Spatial grid ({} cells) exceeded actual cell-buffer capacity ({}); \
                 reconciled to {} cells ({}x{}x{}) at cell_size={:.2} to prevent IMA",
                num_grid_cells,
                cell_capacity,
                num_grid_cells,
                grid_dims.x,
                grid_dims.y,
                grid_dims.z,
                auto_tuned_cell_size
            );
        }

        crate::utils::gpu_diagnostics::validate_kernel_launch(
            self.build_grid_kernel_name,
            grid_size,
            block_size,
            self.num_nodes,
        )
        .map_err(|e| anyhow::anyhow!(e))?;
        let build_grid_kernel = self
            ._module
            .get_function(self.build_grid_kernel_name)
            .map_err(|e| {
                let diagnosis = crate::utils::gpu_diagnostics::diagnose_ptx_error(&format!(
                    "Kernel '{}' not found: {}",
                    self.build_grid_kernel_name, e
                ));
                anyhow!(
                    "Failed to get kernel function '{}':\n{}",
                    self.build_grid_kernel_name,
                    diagnosis
                )
            })?;
        // SAFETY: Grid building kernel launch is safe because:
        // 1. pos_in_* buffers are valid DeviceBuffers with capacity >= num_nodes
        // 2. cell_keys buffer is sized for allocated_nodes elements
        // 3. aabb and grid_dims are computed from valid position data
        // 4. auto_tuned_cell_size is a positive float computed from AABB dimensions
        // 5. validate_kernel_launch() was called above to verify launch parameters
        unsafe {
            let stream = &self.stream;
            launch!(
                build_grid_kernel<<<grid_size as u32, block_size as u32, 0, stream>>>(
                self.pos_in_x.as_device_ptr(),
                self.pos_in_y.as_device_ptr(),
                self.pos_in_z.as_device_ptr(),
                self.cell_keys.as_device_ptr(),
                aabb,
                grid_dims,
                auto_tuned_cell_size,
                self.num_nodes as i32,
                num_grid_cells as i32
            ))?;
        }

        // Persistent grid-sort output buffers (allocated once in new()/resize_buffers,
        // reused every frame). `sort_keys_out` receives the sorted cell keys;
        // `sort_values_out` receives the sorted node indices and is ping-ponged with
        // `sorted_node_indices` via the swap below.
        let keys_in_ptr = self.cell_keys.as_device_ptr().as_raw() as *const ::std::os::raw::c_void;
        let values_in_ptr =
            self.sorted_node_indices.as_device_ptr().as_raw() as *const ::std::os::raw::c_void;
        let keys_out_ptr =
            self.sort_keys_out.as_device_ptr().as_raw() as *mut ::std::os::raw::c_void;
        let values_out_ptr =
            self.sort_values_out.as_device_ptr().as_raw() as *mut ::std::os::raw::c_void;

        // SAFETY: Thrust sort FFI call is safe because:
        // 1. cell_keys is a valid DeviceBuffer allocated for allocated_nodes elements
        // 2. sort_keys_out is a persistent DeviceBuffer sized allocated_nodes (zeroed at alloc)
        // 3. sorted_node_indices is a valid DeviceBuffer for allocated_nodes elements
        // 4. sort_values_out is a persistent DeviceBuffer sized allocated_nodes (zeroed at alloc)
        // 5. num_items is bounded by min(num_nodes, allocated_nodes) preventing out-of-bounds
        // 6. stream_ptr is obtained from a valid cust::Stream via as_inner()
        // 7. Thrust internally synchronizes on the provided stream before returning
        unsafe {
            let stream_ptr = self.stream.as_inner() as *mut ::std::os::raw::c_void;
            thrust_sort_key_value(
                keys_in_ptr,
                keys_out_ptr,
                values_in_ptr,
                values_out_ptr,
                self.num_nodes.min(self.allocated_nodes) as ::std::os::raw::c_int,
                stream_ptr,
            );
        }

        // Sorted node indices now live in sort_values_out; swap them into
        // sorted_node_indices for downstream kernels (ping-pong, no allocation).
        std::mem::swap(&mut self.sorted_node_indices, &mut self.sort_values_out);

        if self.cell_start.len() != self.zero_buffer.len() {
            return Err(anyhow!(
                "cell_start/zero_buffer size mismatch: cell_start={} elements, zero_buffer={} elements, max_grid_cells={}, num_grid_cells={}",
                self.cell_start.len(), self.zero_buffer.len(), self.max_grid_cells, num_grid_cells
            ));
        }
        safe_copy_to_device(&mut self.cell_start, &self.zero_buffer, "cell_start")?;
        safe_copy_to_device(&mut self.cell_end, &self.zero_buffer, "cell_end")?;

        let cell_block_size = block_size;
        let grid_cells_blocks = (num_grid_cells as u32 + cell_block_size - 1) / cell_block_size;
        let compute_cell_bounds_kernel = self
            ._module
            .get_function(self.compute_cell_bounds_kernel_name)?;
        // SAFETY: Cell bounds kernel launch is safe because:
        // 1. sort_keys_out is the output from thrust_sort_key_value (valid device memory)
        // 2. cell_start and cell_end were zeroed and have capacity >= num_grid_cells
        // 3. num_grid_cells was computed from validated grid dimensions
        // 4. The kernel reads sort_keys_out and writes cell boundaries atomically
        unsafe {
            let stream = &self.stream;
            launch!(
                compute_cell_bounds_kernel<<<grid_cells_blocks, cell_block_size, 0, stream>>>(
                self.sort_keys_out.as_device_ptr(),
                self.cell_start.as_device_ptr(),
                self.cell_end.as_device_ptr(),
                self.num_nodes as i32,
                num_grid_cells as i32
            ))?;
        }

        let force_kernel_name = if params.stability_threshold > 0.0 {
            "force_pass_with_stability_kernel"
        } else {
            self.force_pass_kernel_name
        };
        let force_pass_kernel = self._module.get_function(force_kernel_name)?;
        let stream = &self.stream;

        let d_sssp = if (self.sssp_available || self.sssp_device_distances.is_some())
            && (params.feature_flags
                & crate::models::simulation_params::FeatureFlags::ENABLE_SSSP_SPRING_ADJUST
                != 0)
        {
            // Prefer the persistent sssp_device_distances buffer (stable across run_sssp calls)
            // over self.dist which is the working buffer that gets overwritten each SSSP run.
            match &self.sssp_device_distances {
                Some(buf) => buf.as_device_ptr(),
                None => self.dist.as_device_ptr(),
            }
        } else {
            DevicePointer::null()
        };

        // SAFETY: Force computation kernel launch is safe because:
        // 1. All position, velocity, and force buffers are valid DeviceBuffers with capacity >= num_nodes
        // 2. cell_start, cell_end, sorted_node_indices, cell_keys are from the spatial grid build phase
        // 3. edge_row_offsets, edge_col_indices, edge_weights are CSR graph data loaded at construction
        // 4. d_sssp is either a valid DevicePointer to dist buffer or DevicePointer::null()
        // 5. constraint_data has capacity for num_constraints ConstraintData elements
        // 6. should_skip_physics is a valid single-element DeviceBuffer for stability gating
        // 7. grid_size and block_size are validated via validate_kernel_launch()
        // FA2: pass node_degrees when available, otherwise null (falls back to classic repulsion)
        let d_node_degrees = if self.degree_weights_available {
            self.node_degrees.as_device_ptr()
        } else {
            DevicePointer::<f32>::null()
        };

        unsafe {
            if params.stability_threshold > 0.0 {
                // Force pass with stability checking variant
                launch!(
                    force_pass_kernel<<<grid_size as u32, block_size as u32, 0, stream>>>(
                    self.pos_in_x.as_device_ptr(),
                    self.pos_in_y.as_device_ptr(),
                    self.pos_in_z.as_device_ptr(),
                    self.vel_in_x.as_device_ptr(),
                    self.vel_in_y.as_device_ptr(),
                    self.vel_in_z.as_device_ptr(),
                    self.force_x.as_device_ptr(),
                    self.force_y.as_device_ptr(),
                    self.force_z.as_device_ptr(),
                    self.cell_start.as_device_ptr(),
                    self.cell_end.as_device_ptr(),
                    self.sorted_node_indices.as_device_ptr(),
                    self.cell_keys.as_device_ptr(),
                    grid_dims,
                    self.edge_row_offsets.as_device_ptr(),
                    self.edge_col_indices.as_device_ptr(),
                    self.edge_weights.as_device_ptr(),
                    self.num_nodes as i32,
                    d_sssp,
                    self.constraint_data.as_device_ptr(),
                    self.num_constraints as i32,
                    self.should_skip_physics.as_device_ptr(),
                    d_node_degrees,
                    self.spring_scale.as_device_ptr()
                ))?;
            } else {
                launch!(
                    force_pass_kernel<<<grid_size as u32, block_size as u32, 0, stream>>>(
                    self.pos_in_x.as_device_ptr(),
                    self.pos_in_y.as_device_ptr(),
                    self.pos_in_z.as_device_ptr(),
                    self.force_x.as_device_ptr(),
                    self.force_y.as_device_ptr(),
                    self.force_z.as_device_ptr(),
                    self.cell_start.as_device_ptr(),
                    self.cell_end.as_device_ptr(),
                    self.sorted_node_indices.as_device_ptr(),
                    self.cell_keys.as_device_ptr(),
                    grid_dims,
                    self.edge_row_offsets.as_device_ptr(),
                    self.edge_col_indices.as_device_ptr(),
                    self.edge_weights.as_device_ptr(),
                    self.num_nodes as i32,
                    d_sssp,
                    self.constraint_data.as_device_ptr(),
                    self.num_constraints as i32,
                    DevicePointer::<f32>::null(),
                    DevicePointer::<f32>::null(),
                    DevicePointer::<f32>::null(),
                    // Ontology class metadata
                    self.class_id.as_device_ptr(),
                    self.class_charge.as_device_ptr(),
                    self.class_mass.as_device_ptr(),
                    // FA2 degree-scaled repulsion
                    d_node_degrees,
                    // Per-population spring strength multiplier
                    self.spring_scale.as_device_ptr()
                ))?;
            }
        }

        // Cluster cohesion: gentle attraction toward cluster centroids.
        // cluster_strength IS the raw kernel coefficient — no magic scale; clamp
        // to the valid contract range [0, 0.02]. The slider has full authority.
        let cohesion_strength = params.cluster_strength.clamp(0.0, 0.02);
        if cohesion_strength > 0.0001 {
            // Community-driven cohesion (Leiden default / Louvain). Communities are
            // topology-derived (modularity over CSR adjacency), so attraction follows
            // graph STRUCTURE; labels refresh on a cadence (host round-trips) while
            // centroids recompute every frame from live positions. K-means spatial
            // clustering is an analytics concern (coloring / hulls), not a cohesion force.
            {
                let need_refresh = self.community_count_active == 0
                    || (self.iteration - self.last_cohesion_refresh_iter)
                        >= self.cohesion_refresh_interval as i32;
                if need_refresh {
                    if let Err(e) = self.refresh_community_cohesion_labels() {
                        log::warn!("[CohesionLouvain] label refresh failed: {}", e);
                    }
                    // Throttle to the cadence on both success and error so a
                    // persistent failure does not re-run Louvain every frame.
                    self.last_cohesion_refresh_iter = self.iteration;
                }

                // Only apply when a meaningful partition exists (>1 community).
                if self.community_count_active > 1 {
                    let ncomm = self.community_count_active;

                    // (a) per-community centroids from live positions. Reuses the
                    // K-means update_centroids_kernel: one block per community,
                    // shared-mem reduction writes mean position + count.
                    if let Ok(update_kernel) = self._module.get_function("update_centroids_kernel")
                    {
                        let centroid_shared_memory = block_size as u32 * (3 * 4 + 4);
                        let stream = &self.stream;
                        unsafe {
                            launch!(
                                update_kernel<<<ncomm as u32, block_size as u32, centroid_shared_memory, stream>>>(
                                self.pos_in_x.as_device_ptr(),
                                self.pos_in_y.as_device_ptr(),
                                self.pos_in_z.as_device_ptr(),
                                self.cluster_assignments.as_device_ptr(),
                                self.community_centroids_x.as_device_ptr(),
                                self.community_centroids_y.as_device_ptr(),
                                self.community_centroids_z.as_device_ptr(),
                                self.community_sizes.as_device_ptr(),
                                self.num_nodes as i32,
                                ncomm as i32
                            ))?;
                        }
                    }

                    // (b) pull each node toward its community centroid. Same kernel
                    // as K-means cohesion, fed community labels + centroids; the
                    // kernel guards `cluster_assignments[i] < num_clusters`.
                    if let Ok(cohesion_kernel) =
                        self._module.get_function("cluster_cohesion_kernel")
                    {
                        let stream = &self.stream;
                        unsafe {
                            launch!(
                                cohesion_kernel<<<grid_size as u32, block_size as u32, 0, stream>>>(
                                self.pos_in_x.as_device_ptr(),
                                self.pos_in_y.as_device_ptr(),
                                self.pos_in_z.as_device_ptr(),
                                self.force_x.as_device_ptr(),
                                self.force_y.as_device_ptr(),
                                self.force_z.as_device_ptr(),
                                self.community_centroids_x.as_device_ptr(),
                                self.community_centroids_y.as_device_ptr(),
                                self.community_centroids_z.as_device_ptr(),
                                self.cluster_assignments.as_device_ptr(),
                                self.num_nodes as i32,
                                ncomm as i32,
                                cohesion_strength
                            ))?;
                        }
                    }
                }
            }
        }

        // Degree-weighted gravity correction: replaces uniform centering with
        // degree-aware gravity for connected nodes and peripheral shell force
        // for isolated nodes. Only runs when degree weights have been uploaded.
        if self.degree_weights_available && params.center_gravity_k > 0.0 {
            if let Ok(dw_gravity_kernel) =
                self._module.get_function("degree_weighted_gravity_kernel")
            {
                // Compute peripheral radius as 2x the average connected-node distance from origin.
                // We use a simple heuristic: 2x the AABB extent diagonal / 2.
                let extent_x = aabb.max[0] - aabb.min[0];
                let extent_y = aabb.max[1] - aabb.min[1];
                let extent_z = aabb.max[2] - aabb.min[2];
                let peripheral_radius =
                    (extent_x * extent_x + extent_y * extent_y + extent_z * extent_z).sqrt();
                let isolated_spring_k = 0.01f32; // Gentle spring toward peripheral shell

                let stream = &self.stream;
                // SAFETY: Degree-weighted gravity kernel launch is safe because:
                // 1. pos_in_* buffers are valid DeviceBuffers with capacity >= num_nodes
                // 2. force_* buffers contain accumulated forces from the force pass
                // 3. degree_weight buffer was uploaded via upload_degree_weights()
                // 4. All scalar parameters are finite floats
                unsafe {
                    launch!(
                        dw_gravity_kernel<<<grid_size as u32, block_size as u32, 0, stream>>>(
                        self.pos_in_x.as_device_ptr(),
                        self.pos_in_y.as_device_ptr(),
                        self.pos_in_z.as_device_ptr(),
                        self.force_x.as_device_ptr(),
                        self.force_y.as_device_ptr(),
                        self.force_z.as_device_ptr(),
                        self.degree_weight.as_device_ptr(),
                        self.num_nodes as i32,
                        params.center_gravity_k,
                        peripheral_radius,
                        isolated_spring_k
                    ))?;
                }
            }
        }

        let integrate_pass_kernel = self._module.get_function(self.integrate_pass_kernel_name)?;
        let stream = &self.stream;
        // SAFETY: Integration kernel launch is safe because:
        // 1. All input buffers (pos_in_*, vel_in_*, force_*, mass) contain data from force pass
        // 2. All output buffers (pos_out_*, vel_out_*) are valid DeviceBuffers with capacity >= num_nodes
        // 3. class_id, class_charge, class_mass are ontology metadata buffers loaded at construction
        // 4. The kernel performs Verlet integration using c_params constants from device memory
        // 5. After this kernel, swap_buffers() exchanges input/output for next iteration
        unsafe {
            launch!(
                integrate_pass_kernel<<<grid_size as u32, block_size as u32, 0, stream>>>(
                self.pos_in_x.as_device_ptr(),
                self.pos_in_y.as_device_ptr(),
                self.pos_in_z.as_device_ptr(),
                self.vel_in_x.as_device_ptr(),
                self.vel_in_y.as_device_ptr(),
                self.vel_in_z.as_device_ptr(),
                self.force_x.as_device_ptr(),
                self.force_y.as_device_ptr(),
                self.force_z.as_device_ptr(),
                self.mass.as_device_ptr(),
                self.pos_out_x.as_device_ptr(),
                self.pos_out_y.as_device_ptr(),
                self.pos_out_z.as_device_ptr(),
                self.vel_out_x.as_device_ptr(),
                self.vel_out_y.as_device_ptr(),
                self.vel_out_z.as_device_ptr(),
                self.num_nodes as i32,
                // Ontology class metadata
                self.class_id.as_device_ptr(),
                self.class_charge.as_device_ptr(),
                self.class_mass.as_device_ptr(),
                // FA2 adaptive speed: previous-step forces for swing/traction
                self.prev_force_x.as_device_ptr(),
                self.prev_force_y.as_device_ptr(),
                self.prev_force_z.as_device_ptr()
            ))?;
        }

        let completion_event = cust::event::Event::new(cust::event::EventFlags::DEFAULT)?;
        completion_event.record(&self.stream)?;

        let poll_start = std::time::Instant::now();
        while completion_event
            .query()
            .unwrap_or(cust::event::EventStatus::Ready)
            != cust::event::EventStatus::Ready
        {
            if poll_start.elapsed() > std::time::Duration::from_secs(10) {
                return Err(anyhow::anyhow!("GPU kernel execution timed out after 10s"));
            }
            std::thread::yield_now();
        }

        self.swap_buffers();
        self.iteration += 1;

        if self.iteration % 100 == 0 {
            let (memory_used, utilization, resize_count) = self.get_memory_metrics();
            let grid_occupancy = self.get_grid_occupancy(num_grid_cells);
            info!("Performance metrics [iter {}]: Memory: {:.1}MB ({:.1}% utilized), Grid occupancy: {:.1}%, Resizes: {}",
                  self.iteration, memory_used as f32 / 1024.0 / 1024.0,
                  utilization * 100.0, grid_occupancy * 100.0, resize_count);
        }

        Ok(())
    }

    pub fn execute_physics_step(
        &mut self,
        params: &crate::models::simulation_params::SimulationParams,
    ) -> Result<()> {
        self.execute_physics_step_with_bypass(params, false)
    }

    pub fn execute_physics_step_with_bypass(
        &mut self,
        params: &crate::models::simulation_params::SimulationParams,
        stability_bypass: bool,
    ) -> Result<()> {
        // Build feature_flags from the SimulationParams and runtime toggles,
        // mirroring the logic in SimulationParams::to_sim_params().
        let mut feature_flags: u32 = 0;
        if params.repel_k > 0.0 {
            feature_flags |= crate::models::simulation_params::FeatureFlags::ENABLE_REPULSION;
        }
        if params.spring_k > 0.0 {
            feature_flags |= crate::models::simulation_params::FeatureFlags::ENABLE_SPRINGS;
        }
        if params.center_gravity_k > 0.0 {
            feature_flags |= crate::models::simulation_params::FeatureFlags::ENABLE_CENTERING;
        }
        // Honour both the SimulationParams flag and the runtime toggle
        if params.use_sssp_distances || self.sssp_spring_adjust_enabled {
            feature_flags |=
                crate::models::simulation_params::FeatureFlags::ENABLE_SSSP_SPRING_ADJUST;
        }
        // KEYSTONE (ADR-098 break #1): gate the live force_pass_kernel constraint
        // loop on. Without this bit the loop at visionclaw_unified.cu:475 never
        // runs, so the uploaded ontology ConstraintData buffer has zero effect.
        // Bit 4 (ENABLE_CONSTRAINTS) is set whenever constraints are resident.
        if self.num_constraints > 0 {
            feature_flags |= crate::models::simulation_params::FeatureFlags::ENABLE_CONSTRAINTS;
        }

        // Use SimulationParams::to_sim_params() which correctly maps ALL user-facing
        // settings to the GPU-compatible SimParams struct. Previous implementation
        // hardcoded many values (temperature, separation_radius, repulsion_cutoff, etc.)
        // which caused "nothing moves when I change settings" because those settings
        // never reached the GPU kernel.
        let mut sim_params = params.to_sim_params();
        sim_params.feature_flags = feature_flags;

        // When stability_bypass is true, disable the GPU stability check so physics
        // runs unconditionally. This prevents the check_system_stability_kernel from
        // skipping physics when the system was at equilibrium before a parameter change.
        if stability_bypass {
            sim_params.stability_threshold = 0.0;
        }

        // Log GPU params on first iteration to confirm forces are enabled
        if self.iteration == 0 {
            info!(
                "GPU execute_physics_step: FIRST iter — feature_flags=0b{:b} (repel={}, spring={}, center={}), dt={}, repel_k={}, spring_k={}, damping={}, stability_bypass={}",
                feature_flags,
                feature_flags & 1 != 0,
                feature_flags & 2 != 0,
                feature_flags & 4 != 0,
                sim_params.dt, sim_params.repel_k, sim_params.spring_k,
                sim_params.damping, stability_bypass
            );
        }

        self.execute(sim_params)
    }

    pub fn get_node_positions(&mut self) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        let _thread_context = Context::new(self.device.clone())
            .map_err(|e| anyhow!("Failed to set CUDA context: {}", e))?;

        let mut pos_x = vec![0.0f32; self.allocated_nodes];
        let mut pos_y = vec![0.0f32; self.allocated_nodes];
        let mut pos_z = vec![0.0f32; self.allocated_nodes];

        safe_copy_from_device(&self.pos_in_x, &mut pos_x, "pos_in_x")?;
        safe_copy_from_device(&self.pos_in_y, &mut pos_y, "pos_in_y")?;
        safe_copy_from_device(&self.pos_in_z, &mut pos_z, "pos_in_z")?;

        pos_x.truncate(self.num_nodes);
        pos_y.truncate(self.num_nodes);
        pos_z.truncate(self.num_nodes);

        Ok((pos_x, pos_y, pos_z))
    }

    pub fn get_node_velocities(&mut self) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        let _thread_context = Context::new(self.device.clone())
            .map_err(|e| anyhow!("Failed to set CUDA context: {}", e))?;

        let mut vel_x = vec![0.0f32; self.allocated_nodes];
        let mut vel_y = vec![0.0f32; self.allocated_nodes];
        let mut vel_z = vec![0.0f32; self.allocated_nodes];

        safe_copy_from_device(&self.vel_in_x, &mut vel_x, "vel_in_x")?;
        safe_copy_from_device(&self.vel_in_y, &mut vel_y, "vel_in_y")?;
        safe_copy_from_device(&self.vel_in_z, &mut vel_z, "vel_in_z")?;

        vel_x.truncate(self.num_nodes);
        vel_y.truncate(self.num_nodes);
        vel_z.truncate(self.num_nodes);

        Ok((vel_x, vel_y, vel_z))
    }

    /// Inject random velocity perturbation to break equilibrium after param changes.
    /// `factor` scales magnitude (0.3 = mild re-layout, 1.0 = strong shake).
    pub fn inject_velocity_perturbation(&mut self, factor: f32) -> Result<()> {
        // Bind the primary CUDA context to this thread. Called from spawn_blocking
        // worker threads which do not inherit the context, so the device copies below
        // would otherwise fail with CUDA_ERROR_INVALID_CONTEXT and poison the step.
        let _thread_context = Context::new(self.device.clone())
            .map_err(|e| anyhow!("Failed to set CUDA context: {}", e))?;
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = self.num_nodes.min(self.allocated_nodes);
        let mut vx = vec![0.0f32; self.allocated_nodes];
        let mut vy = vec![0.0f32; self.allocated_nodes];
        let mut vz = vec![0.0f32; self.allocated_nodes];
        safe_copy_from_device(&self.vel_in_x, &mut vx, "vel_in_x")?;
        safe_copy_from_device(&self.vel_in_y, &mut vy, "vel_in_y")?;
        safe_copy_from_device(&self.vel_in_z, &mut vz, "vel_in_z")?;
        let magnitude = factor * 2.0;
        for i in 0..n {
            vx[i] += rng.gen_range(-magnitude..magnitude);
            vy[i] += rng.gen_range(-magnitude..magnitude);
            vz[i] += rng.gen_range(-magnitude..magnitude);
        }
        safe_copy_to_device(&mut self.vel_in_x, &vx, "vel_in_x")?;
        safe_copy_to_device(&mut self.vel_in_y, &vy, "vel_in_y")?;
        safe_copy_to_device(&mut self.vel_in_z, &vz, "vel_in_z")?;
        Ok(())
    }

    /// Zero all node velocities on the GPU. Used by the divergence circuit
    /// breaker to drain runaway kinetic energy so the layout can re-settle from
    /// its restored (last-known-good) positions instead of re-exploding.
    pub fn reset_velocities(&mut self) -> Result<()> {
        let _thread_context = Context::new(self.device.clone())
            .map_err(|e| anyhow!("Failed to set CUDA context: {}", e))?;
        let zeros = vec![0.0f32; self.allocated_nodes];
        safe_copy_to_device(&mut self.vel_in_x, &zeros, "vel_in_x")?;
        safe_copy_to_device(&mut self.vel_in_y, &zeros, "vel_in_y")?;
        safe_copy_to_device(&mut self.vel_in_z, &zeros, "vel_in_z")?;
        Ok(())
    }
}
