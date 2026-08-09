// src/gpu/types.rs
//! Canonical GPU Type Definitions
//!
//! This module contains the authoritative struct definitions for GPU operations.
//! All other modules should import from here to ensure consistency.

use crate::utils::gpu_safety::GPUSafetyError;
use serde::{Deserialize, Serialize};

// =============================================================================
// RenderData - CANONICAL DEFINITION
// =============================================================================

/// Canonical GPU render data structure used for streaming and visual analytics
/// This is the **authoritative** definition. Other modules must import this type.
/// # Layout
/// - `positions`: Vec<f32> with length = num_nodes * 4 (x, y, z, w components)
/// - `colors`: Vec<f32> with length = num_nodes * 4 (r, g, b, a components)
/// - `importance`: Vec<f32> with length = num_nodes (importance scores)
/// - `frame`: u32 frame number
/// # Used By
/// - src/gpu/streaming_pipeline.rs
/// - src/gpu/visual_analytics.rs
/// - src/gpu/conversion_utils.rs
/// # Validation
/// Use `validate()` before GPU operations to ensure data integrity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderData {
    /// Node positions as [x, y, z, w] components (w typically 1.0)
    pub positions: Vec<f32>,

    /// Node colors as [r, g, b, a] components
    pub colors: Vec<f32>,

    /// Per-node importance scores (0.0 to 1.0)
    pub importance: Vec<f32>,

    /// Frame number for synchronization
    pub frame: u32,
}

impl RenderData {
    /// Create new validated RenderData
    pub fn new(
        positions: Vec<f32>,
        colors: Vec<f32>,
        importance: Vec<f32>,
        frame: u32,
    ) -> Result<Self, GPUSafetyError> {
        let data = Self {
            positions,
            colors,
            importance,
            frame,
        };
        data.validate()?;
        Ok(data)
    }

    /// Validate RenderData structure
    pub fn validate(&self) -> Result<(), GPUSafetyError> {
        use crate::gpu::conversion_utils::validate_render_data;

        validate_render_data(&self.positions, &self.colors, &self.importance)
            .map(|_node_count| ())
            .map_err(|e| GPUSafetyError::InvalidKernelParams {
                reason: format!("RenderData validation failed: {}", e),
            })?;

        // Additional validation: check for non-finite values
        for (i, &val) in self.positions.iter().enumerate() {
            if !val.is_finite() {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid position value at index {}: {}", i, val),
                });
            }
        }

        for (i, &val) in self.colors.iter().enumerate() {
            if !val.is_finite() {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid color value at index {}: {}", i, val),
                });
            }
        }

        for (i, &val) in self.importance.iter().enumerate() {
            if !val.is_finite() || val < 0.0 {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid importance value at index {}: {}", i, val),
                });
            }
        }

        Ok(())
    }

    /// Get the number of nodes represented
    pub fn node_count(&self) -> usize {
        self.positions.len() / 4
    }

    /// Create empty RenderData for a given number of nodes
    pub fn empty(num_nodes: usize) -> Self {
        Self {
            positions: vec![0.0; num_nodes * 4],
            colors: vec![0.0; num_nodes * 4],
            importance: vec![0.0; num_nodes],
            frame: 0,
        }
    }
}

// =============================================================================
// BinaryNodeData - CANONICAL DEFINITION
// =============================================================================

/// Canonical binary node data structure for network transmission and GPU operations
/// This replaces multiple duplicate definitions across the codebase.
/// # Layout (28 bytes)
/// - node_id: u32 (4 bytes)
/// - x, y, z: f32 (12 bytes)
/// - vx, vy, vz: f32 (12 bytes)
/// # Used By
/// - src/utils/socket_flow_messages.rs
/// - src/utils/binary_protocol.rs
/// - GPU streaming operations
/// # See Also
/// - BinaryNodeDataGPU for extended GPU-side data
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BinaryNodeData {
    pub node_id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
}

impl BinaryNodeData {
    pub fn new(node_id: u32, position: [f32; 3], velocity: [f32; 3]) -> Self {
        Self {
            node_id,
            x: position[0],
            y: position[1],
            z: position[2],
            vx: velocity[0],
            vy: velocity[1],
            vz: velocity[2],
        }
    }

    pub fn position(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn velocity(&self) -> [f32; 3] {
        [self.vx, self.vy, self.vz]
    }

    pub fn validate(&self) -> Result<(), GPUSafetyError> {
        // Check for finite values
        let values = [self.x, self.y, self.z, self.vx, self.vy, self.vz];
        for (i, &val) in values.iter().enumerate() {
            if !val.is_finite() {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("BinaryNodeData: non-finite value at index {}: {}", i, val),
                });
            }
        }

        // Check for reasonable bounds (prevent overflow)
        const MAX_COORD: f32 = 1e6;
        if self.x.abs() > MAX_COORD || self.y.abs() > MAX_COORD || self.z.abs() > MAX_COORD {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "BinaryNodeData: position coordinates exceed safe bounds: ({}, {}, {})",
                    self.x, self.y, self.z
                ),
            });
        }

        Ok(())
    }
}

// =============================================================================
// Static Assertions for FFI Struct Sizes and Alignment
// =============================================================================
//
// These compile-time checks ensure Rust struct sizes match C/C++ definitions.
// BinaryNodeData is used in network protocol and GPU streaming operations.

use static_assertions::const_assert_eq;

// BinaryNodeData: 28 bytes total
// - node_id: u32 (4 bytes)
// - x, y, z: f32 (12 bytes)
// - vx, vy, vz: f32 (12 bytes)
const_assert_eq!(std::mem::size_of::<BinaryNodeData>(), 28);
const_assert_eq!(std::mem::align_of::<BinaryNodeData>(), 4);

// =============================================================================
// Migration Helpers
// =============================================================================

/// Migration helper for code using old import paths
pub mod legacy {
    use super::*;

    /// Re-export for backwards compatibility with streaming_pipeline
    pub type StreamingPipelineRenderData = RenderData;

    /// Re-export for backwards compatibility with visual_analytics
    pub type VisualAnalyticsRenderData = RenderData;

    /// Re-export for backwards compatibility with socket messages
    pub type BinaryNodeDataClient = BinaryNodeData;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_data_validation() {
        // Valid data
        let valid_data = RenderData {
            positions: vec![1.0f32; 40], // 10 nodes * 4
            colors: vec![0.5f32; 40],
            importance: vec![0.8f32; 10],
            frame: 1,
        };
        assert!(valid_data.validate().is_ok());

        // Invalid: positions not divisible by 4
        let invalid_data = RenderData {
            positions: vec![1.0f32; 39],
            colors: vec![0.5f32; 40],
            importance: vec![0.8f32; 10],
            frame: 1,
        };
        assert!(invalid_data.validate().is_err());

        // Invalid: mismatched node counts
        let mismatched_data = RenderData {
            positions: vec![1.0f32; 40],
            colors: vec![0.5f32; 40],
            importance: vec![0.8f32; 15], // Wrong count
            frame: 1,
        };
        assert!(mismatched_data.validate().is_err());

        // Invalid: NaN value
        let mut nan_data = RenderData {
            positions: vec![1.0f32; 40],
            colors: vec![0.5f32; 40],
            importance: vec![0.8f32; 10],
            frame: 1,
        };
        nan_data.positions[0] = f32::NAN;
        assert!(nan_data.validate().is_err());
    }

    #[test]
    fn test_binary_node_data_validation() {
        // Valid data
        let valid = BinaryNodeData::new(1, [10.0, 20.0, 30.0], [1.0, 2.0, 3.0]);
        assert!(valid.validate().is_ok());

        // Invalid: NaN
        let invalid = BinaryNodeData {
            node_id: 1,
            x: f32::NAN,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
        };
        assert!(invalid.validate().is_err());

        // Invalid: extreme coordinates
        let extreme = BinaryNodeData {
            node_id: 1,
            x: 1e7,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
        };
        assert!(extreme.validate().is_err());
    }

    #[test]
    fn test_render_data_node_count() {
        let data = RenderData::empty(100);
        assert_eq!(data.node_count(), 100);
        assert_eq!(data.positions.len(), 400);
        assert_eq!(data.colors.len(), 400);
        assert_eq!(data.importance.len(), 100);
    }
}
