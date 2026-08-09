use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Vec3Data {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// ADR-090 Phase 1b: bridging conversions between domain Vec3Data and webxr Vec3Data.
// Both are plain (x, y, z) f32 triples — the only difference is bytemuck derives.
impl From<visionclaw_domain::Vec3Data> for Vec3Data {
    fn from(v: visionclaw_domain::Vec3Data) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<Vec3Data> for visionclaw_domain::Vec3Data {
    fn from(v: Vec3Data) -> Self {
        visionclaw_domain::Vec3Data::new(v.x, v.y, v.z)
    }
}

impl From<Vec3> for Vec3Data {
    fn from(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<Vec3Data> for Vec3 {
    fn from(v: Vec3Data) -> Self {
        Vec3::new(v.x, v.y, v.z)
    }
}

impl From<[f32; 3]> for Vec3Data {
    fn from(arr: [f32; 3]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
            z: arr[2],
        }
    }
}

impl From<Vec3Data> for [f32; 3] {
    fn from(v: Vec3Data) -> Self {
        [v.x, v.y, v.z]
    }
}

impl Vec3Data {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn as_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn as_vec3(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec3_data_conversions() {
        let vec3 = Vec3::new(1.0, 2.0, 3.0);
        let vec3_data: Vec3Data = vec3.into();
        let array: [f32; 3] = vec3_data.into();
        let back_to_vec3: Vec3 = vec3_data.into();

        assert_eq!(vec3_data.x, 1.0);
        assert_eq!(vec3_data.y, 2.0);
        assert_eq!(vec3_data.z, 3.0);
        assert_eq!(array, [1.0, 2.0, 3.0]);
        assert_eq!(back_to_vec3, vec3);
    }

    #[test]
    fn test_array_conversion() {
        let array = [1.0, 2.0, 3.0];
        let vec3_data: Vec3Data = array.into();
        let back_to_array: [f32; 3] = vec3_data.into();

        assert_eq!(vec3_data.x, 1.0);
        assert_eq!(vec3_data.y, 2.0);
        assert_eq!(vec3_data.z, 3.0);
        assert_eq!(back_to_array, array);
    }

    #[test]
    fn test_zero() {
        let zero = Vec3Data::zero();
        assert_eq!(zero.x, 0.0);
        assert_eq!(zero.y, 0.0);
        assert_eq!(zero.z, 0.0);
    }
}
