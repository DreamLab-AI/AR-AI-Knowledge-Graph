use serde::{Deserialize, Serialize};
use specta::Type;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Clone, Default, Type)]
#[serde(rename_all = "camelCase")]
pub struct MovementAxes {
    #[serde(alias = "horizontal")]
    pub horizontal: i32,
    #[serde(alias = "vertical")]
    pub vertical: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, Type, Validate)]
#[serde(rename_all = "camelCase")]
pub struct XRSettings {
    #[serde(skip_serializing_if = "Option::is_none", alias = "enabled")]
    pub enabled: Option<bool>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "client_side_enable_xr"
    )]
    pub client_side_enable_xr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "mode")]
    pub mode: Option<String>,
    #[serde(alias = "room_scale")]
    pub room_scale: f32,
    #[serde(alias = "space_type")]
    pub space_type: String,
    #[serde(alias = "quality")]
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none", alias = "render_scale")]
    pub render_scale: Option<f32>,
    #[serde(alias = "interaction_distance")]
    pub interaction_distance: f32,
    #[serde(alias = "locomotion_method")]
    pub locomotion_method: String,
    #[serde(alias = "teleport_ray_color")]
    pub teleport_ray_color: String,
    #[serde(alias = "controller_ray_color")]
    pub controller_ray_color: String,
    #[serde(skip_serializing_if = "Option::is_none", alias = "controller_model")]
    pub controller_model: Option<String>,

    #[serde(alias = "enable_hand_tracking")]
    pub enable_hand_tracking: bool,
    #[serde(alias = "hand_mesh_enabled")]
    pub hand_mesh_enabled: bool,
    #[serde(alias = "hand_mesh_color")]
    pub hand_mesh_color: String,
    #[serde(alias = "hand_mesh_opacity")]
    pub hand_mesh_opacity: f32,
    #[serde(alias = "hand_point_size")]
    pub hand_point_size: f32,
    #[serde(alias = "hand_ray_enabled")]
    pub hand_ray_enabled: bool,
    #[serde(alias = "hand_ray_color")]
    pub hand_ray_color: String,
    #[serde(alias = "hand_ray_width")]
    pub hand_ray_width: f32,
    #[serde(alias = "gesture_smoothing")]
    pub gesture_smoothing: f32,

    #[serde(alias = "enable_haptics")]
    pub enable_haptics: bool,
    #[serde(alias = "haptic_intensity")]
    pub haptic_intensity: f32,
    #[serde(alias = "drag_threshold")]
    pub drag_threshold: f32,
    #[serde(alias = "pinch_threshold")]
    pub pinch_threshold: f32,
    #[serde(alias = "rotation_threshold")]
    pub rotation_threshold: f32,
    #[serde(alias = "interaction_radius")]
    pub interaction_radius: f32,
    #[serde(alias = "movement_speed")]
    pub movement_speed: f32,
    #[serde(alias = "dead_zone")]
    pub dead_zone: f32,
    #[serde(alias = "movement_axes")]
    pub movement_axes: MovementAxes,

    #[serde(alias = "enable_light_estimation")]
    pub enable_light_estimation: bool,
    #[serde(alias = "enable_plane_detection")]
    pub enable_plane_detection: bool,
    #[serde(alias = "enable_scene_understanding")]
    pub enable_scene_understanding: bool,
    #[serde(alias = "plane_color")]
    pub plane_color: String,
    #[serde(alias = "plane_opacity")]
    pub plane_opacity: f32,
    #[serde(alias = "plane_detection_distance")]
    pub plane_detection_distance: f32,
    #[serde(alias = "show_plane_overlay")]
    pub show_plane_overlay: bool,
    #[serde(alias = "snap_to_floor")]
    pub snap_to_floor: bool,

    #[serde(alias = "enable_passthrough_portal")]
    pub enable_passthrough_portal: bool,
    #[serde(alias = "passthrough_opacity")]
    pub passthrough_opacity: f32,
    #[serde(alias = "passthrough_brightness")]
    pub passthrough_brightness: f32,
    #[serde(alias = "passthrough_contrast")]
    pub passthrough_contrast: f32,
    #[serde(alias = "portal_size")]
    pub portal_size: f32,
    #[serde(alias = "portal_edge_color")]
    pub portal_edge_color: String,
    #[serde(alias = "portal_edge_width")]
    pub portal_edge_width: f32,
}
