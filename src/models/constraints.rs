// ADR-090 Phase 1b: Domain constraint types (ConstraintKind, Constraint,
// AdvancedParams, ConstraintSet) are re-exported from visionclaw-domain.
// ConstraintData stays here because it requires GPU-specific derives
// (bytemuck::Pod/Zeroable, cust::memory::DeviceCopy) that cannot live
// in the pure-domain crate.
pub use visionclaw_domain::models::constraints::{
    AdvancedParams, Constraint, ConstraintKind, ConstraintSet,
};

/// GPU-buffer representation of a single constraint.
/// Kept in webxr (not domain) because it requires bytemuck + cust GPU traits.
/// Matches the `#[repr(C)]` layout expected by the CUDA `force_pass_kernel`
/// constraint loop in visionclaw_unified.cu (ADR-098 D3: the separate
/// ontology_constraints.cu kernel is retired).
#[repr(C)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    bytemuck::Pod,
    bytemuck::Zeroable,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ConstraintData {
    /// ConstraintKind discriminant cast to i32
    pub kind: i32,
    /// Number of valid entries in node_idx (1–4)
    pub count: i32,
    /// Up to 4 node indices (padded with 0)
    pub node_idx: [i32; 4],
    /// Up to 8 floating-point parameters
    pub params: [f32; 8],
    /// Blend weight
    pub weight: f32,
    /// Frame at which this constraint becomes active (0 = always)
    pub activation_frame: i32,
}

impl Default for ConstraintData {
    fn default() -> Self {
        Self {
            kind: 0,
            count: 0,
            node_idx: [0; 4],
            params: [0.0; 8],
            weight: 0.0,
            activation_frame: 0,
        }
    }
}

// Manual implementation of DeviceCopy for ConstraintData (GPU is always enabled)
unsafe impl cust::memory::DeviceCopy for ConstraintData {}

impl ConstraintData {
    pub fn from_constraint(constraint: &Constraint) -> Self {
        let mut node_idx = [-1i32; 4];
        for (i, &idx) in constraint.node_indices.iter().take(4).enumerate() {
            node_idx[i] = idx as i32;
        }

        let mut params = [0.0f32; 8];
        for (i, &param) in constraint.params.iter().take(8).enumerate() {
            params[i] = param;
        }

        // The domain `ConstraintKind` and the live kernel's `ConstraintKind` are
        // divergently numbered (ADR-098 verified-topology break #3), so a raw
        // `as i32` cast mismaps for the alignment kinds. Translate the three
        // domain alignment variants onto the single live-kernel ALIGNMENT = 7
        // branch, carrying the axis in params[0] (0=x/1=y/2=z) which is what the
        // kernel reads (ADR-141 P4b). Other kinds keep their raw discriminant.
        let kind = match constraint.kind {
            ConstraintKind::AlignmentHorizontal => {
                params[0] = 0.0;
                7
            }
            ConstraintKind::AlignmentVertical => {
                params[0] = 1.0;
                7
            }
            ConstraintKind::AlignmentDepth => {
                params[0] = 2.0;
                7
            }
            other => other as i32,
        };

        Self {
            kind,
            count: constraint.node_indices.len().min(4) as i32,
            node_idx,
            params,
            weight: constraint.weight,
            activation_frame: 0,
        }
    }
}

/// Extension trait that adds GPU-buffer conversion methods to the domain
/// `Constraint` and `ConstraintSet` types.  These methods live here (not in
/// domain) because they produce `ConstraintData`, which requires GPU-specific
/// derives (`bytemuck::Pod/Zeroable`, `cust::DeviceCopy`) that cannot live in
/// the pure-domain crate.
pub trait ConstraintGpuExt {
    fn to_gpu_format(&self) -> ConstraintData;
}

pub trait ConstraintSetGpuExt {
    fn to_gpu_data(&self) -> Vec<ConstraintData>;
}

impl ConstraintGpuExt for Constraint {
    fn to_gpu_format(&self) -> ConstraintData {
        ConstraintData::from_constraint(self)
    }
}

impl ConstraintSetGpuExt for ConstraintSet {
    fn to_gpu_data(&self) -> Vec<ConstraintData> {
        self.active_constraints()
            .into_iter()
            .map(ConstraintData::from_constraint)
            .collect()
    }
}
