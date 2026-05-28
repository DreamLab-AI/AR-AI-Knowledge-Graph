use lazy_static::lazy_static;
use regex::Regex;
use validator::ValidationError;

use super::visualisation::{BloomSettings, GlowSettings};

lazy_static! {
    static ref HEX_COLOR_REGEX: Regex =
        Regex::new(r"^#([A-Fa-f0-9]{6}|[A-Fa-f0-9]{8})$").expect("Invalid regex pattern");

    static ref URL_REGEX: Regex =
        Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").expect("Invalid regex pattern");

    static ref FILE_PATH_REGEX: Regex =
        Regex::new(r"^[a-zA-Z0-9._/\\-]+$").expect("Invalid regex pattern");

    static ref DOMAIN_REGEX: Regex =
        Regex::new(r"^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?)*$")
            .expect("Invalid regex pattern");
}

pub fn validate_hex_color(color: &str) -> Result<(), ValidationError> {
    if !HEX_COLOR_REGEX.is_match(color) {
        return Err(ValidationError::new("invalid_hex_color"));
    }
    Ok(())
}

pub fn validate_width_range(range: &[f32]) -> Result<(), ValidationError> {
    if range.len() != 2 {
        return Err(ValidationError::new("width_range_length"));
    }
    if range[0] >= range[1] {
        return Err(ValidationError::new("width_range_order"));
    }
    Ok(())
}

pub fn validate_port(port: u16) -> Result<(), ValidationError> {
    if port == 0 {
        return Err(ValidationError::new("invalid_port"));
    }
    Ok(())
}

pub fn validate_percentage(value: f32) -> Result<(), ValidationError> {
    if !(0.0..=100.0).contains(&value) {
        return Err(ValidationError::new("invalid_percentage"));
    }
    Ok(())
}

pub fn validate_bloom_glow_settings(
    glow: &GlowSettings,
    bloom: &BloomSettings,
) -> Result<(), ValidationError> {
    if glow.intensity < 0.0 || glow.intensity > 10.0 {
        return Err(ValidationError::new("glow_intensity_out_of_range"));
    }
    if glow.radius < 0.0 || glow.radius > 10.0 {
        return Err(ValidationError::new("glow_radius_out_of_range"));
    }
    if glow.threshold < 0.0 || glow.threshold > 1.0 {
        return Err(ValidationError::new("glow_threshold_out_of_range"));
    }
    if glow.opacity < 0.0 || glow.opacity > 1.0 {
        return Err(ValidationError::new("glow_opacity_out_of_range"));
    }

    validate_hex_color(&glow.base_color)?;
    validate_hex_color(&glow.emission_color)?;

    if !glow.intensity.is_finite() {
        return Err(ValidationError::new("glow_intensity_not_finite"));
    }
    if !glow.radius.is_finite() {
        return Err(ValidationError::new("glow_radius_not_finite"));
    }
    if !glow.threshold.is_finite() {
        return Err(ValidationError::new("glow_threshold_not_finite"));
    }

    if bloom.intensity < 0.0 || bloom.intensity > 10.0 {
        return Err(ValidationError::new("bloom_intensity_out_of_range"));
    }
    if bloom.radius < 0.0 || bloom.radius > 10.0 {
        return Err(ValidationError::new("bloom_radius_out_of_range"));
    }
    if bloom.threshold < 0.0 || bloom.threshold > 1.0 {
        return Err(ValidationError::new("bloom_threshold_out_of_range"));
    }
    if bloom.strength < 0.0 || bloom.strength > 1.0 {
        return Err(ValidationError::new("bloom_strength_out_of_range"));
    }
    if bloom.knee < 0.0 || bloom.knee > 2.0 {
        return Err(ValidationError::new("bloom_knee_out_of_range"));
    }

    validate_hex_color(&bloom.color)?;
    validate_hex_color(&bloom.tint_color)?;

    if !bloom.intensity.is_finite() {
        return Err(ValidationError::new("bloom_intensity_not_finite"));
    }
    if !bloom.radius.is_finite() {
        return Err(ValidationError::new("bloom_radius_not_finite"));
    }
    if !bloom.threshold.is_finite() {
        return Err(ValidationError::new("bloom_threshold_not_finite"));
    }

    Ok(())
}

pub(crate) fn to_camel_case(snake_str: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for ch in snake_str.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}
