//! Centralized JSON serialization/deserialization utilities
//!
//! This module provides standardized JSON operations with consistent error handling
//! to replace the 154+ duplicate serde_json calls throughout the codebase.

use crate::errors::{VisionClawError, VisionClawResult};
use serde::{de::DeserializeOwned, Serialize};

/// Deserialize JSON string into a typed value with standard error handling
/// # Example
/// ```rust,ignore
/// use visionclaw::utils::json::from_json;
/// #[derive(serde::Deserialize)]
/// struct User { name: String }
/// let user: User = from_json(r#"{"name":"Alice"}"#)?;
/// # Ok::<(), visionclaw::errors::VisionClawError>(())
/// ```
pub fn from_json<T: DeserializeOwned>(s: &str) -> VisionClawResult<T> {
    serde_json::from_str(s)
        .map_err(|e| VisionClawError::Serialization(format!("JSON deserialization failed: {}", e)))
}

/// Serialize a value into a JSON string with standard error handling
/// # Example
/// ```rust,ignore
/// use visionclaw::utils::json::to_json;
/// #[derive(serde::Serialize)]
/// struct User { name: String }
/// let user = User { name: "Alice".to_string() };
/// let json = to_json(&user)?;
/// # Ok::<(), visionclaw::errors::VisionClawError>(())
/// ```
pub fn to_json<T: Serialize + ?Sized>(value: &T) -> VisionClawResult<String> {
    serde_json::to_string(value)
        .map_err(|e| VisionClawError::Serialization(format!("JSON serialization failed: {}", e)))
}

/// Deserialize JSON string with custom context for better error messages
/// # Example
/// ```rust,ignore
/// use visionclaw::utils::json::from_json_with_context;
/// #[derive(serde::Deserialize)]
/// struct Config { timeout: u64 }
/// let config: Config = from_json_with_context(
///     r#"{"timeout":5000}"#,
///     "Loading server configuration"
/// )?;
/// # Ok::<(), visionclaw::errors::VisionClawError>(())
/// ```
pub fn from_json_with_context<T: DeserializeOwned>(s: &str, context: &str) -> VisionClawResult<T> {
    serde_json::from_str(s)
        .map_err(|e| VisionClawError::Serialization(format!("{}: {}", context, e)))
}

/// Serialize a value into a pretty-printed JSON string
/// # Example
/// ```rust,ignore
/// use visionclaw::utils::json::to_json_pretty;
/// #[derive(serde::Serialize)]
/// struct User { name: String, age: u32 }
/// let user = User { name: "Alice".to_string(), age: 30 };
/// let json = to_json_pretty(&user)?;
/// assert!(json.contains('\n'));
/// # Ok::<(), visionclaw::errors::VisionClawError>(())
/// ```
pub fn to_json_pretty<T: Serialize>(value: &T) -> VisionClawResult<String> {
    serde_json::to_string_pretty(value).map_err(|e| {
        VisionClawError::Serialization(format!("JSON serialization (pretty) failed: {}", e))
    })
}

/// Deserialize from JSON byte array
/// # Example
/// ```rust,ignore
/// use visionclaw::utils::json::from_json_bytes;
/// #[derive(serde::Deserialize)]
/// struct User { name: String }
/// let bytes = br#"{"name":"Alice"}"#;
/// let user: User = from_json_bytes(bytes)?;
/// # Ok::<(), visionclaw::errors::VisionClawError>(())
/// ```
pub fn from_json_bytes<T: DeserializeOwned>(bytes: &[u8]) -> VisionClawResult<T> {
    serde_json::from_slice(bytes).map_err(|e| {
        VisionClawError::Serialization(format!("JSON deserialization from bytes failed: {}", e))
    })
}

/// Serialize to JSON byte array
/// # Example
/// ```rust,ignore
/// use visionclaw::utils::json::to_json_bytes;
/// #[derive(serde::Serialize)]
/// struct User { name: String }
/// let user = User { name: "Alice".to_string() };
/// let bytes = to_json_bytes(&user)?;
/// # Ok::<(), visionclaw::errors::VisionClawError>(())
/// ```
pub fn to_json_bytes<T: Serialize>(value: &T) -> VisionClawResult<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| {
        VisionClawError::Serialization(format!("JSON serialization to bytes failed: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestStruct {
        name: String,
        age: u32,
        tags: Vec<String>,
    }

    #[test]
    fn test_from_json_success() {
        let json = r#"{"name":"Alice","age":30,"tags":["rust","testing"]}"#;
        let result: VisionClawResult<TestStruct> = from_json(json);

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.name, "Alice");
        assert_eq!(data.age, 30);
        assert_eq!(data.tags.len(), 2);
    }

    #[test]
    fn test_from_json_error() {
        let invalid_json = r#"{"name":"Alice","age":"invalid"}"#;
        let result: VisionClawResult<TestStruct> = from_json(invalid_json);

        assert!(result.is_err());
        if let Err(VisionClawError::Serialization(msg)) = result {
            assert!(msg.contains("JSON deserialization failed"));
        } else {
            panic!("Expected Serialization error");
        }
    }

    #[test]
    fn test_to_json_success() {
        let data = TestStruct {
            name: "Bob".to_string(),
            age: 25,
            tags: vec!["dev".to_string()],
        };

        let result = to_json(&data);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json.contains("Bob"));
        assert!(json.contains("25"));
    }

    #[test]
    fn test_from_json_with_context() {
        let invalid_json = r#"{"invalid"}"#;
        let result: VisionClawResult<TestStruct> =
            from_json_with_context(invalid_json, "Loading user configuration");

        assert!(result.is_err());
        if let Err(VisionClawError::Serialization(msg)) = result {
            assert!(msg.contains("Loading user configuration"));
        } else {
            panic!("Expected Serialization error with context");
        }
    }

    #[test]
    fn test_to_json_pretty() {
        let data = TestStruct {
            name: "Charlie".to_string(),
            age: 35,
            tags: vec!["qa".to_string(), "automation".to_string()],
        };

        let result = to_json_pretty(&data);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json.contains('\n')); // Should have newlines
        assert!(json.contains("Charlie"));
    }

    #[test]
    fn test_from_json_bytes() {
        let json_bytes = br#"{"name":"Dave","age":40,"tags":[]}"#;
        let result: VisionClawResult<TestStruct> = from_json_bytes(json_bytes);

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.name, "Dave");
        assert_eq!(data.age, 40);
    }

    #[test]
    fn test_to_json_bytes() {
        let data = TestStruct {
            name: "Eve".to_string(),
            age: 28,
            tags: vec![],
        };

        let result = to_json_bytes(&data);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        let json_str = String::from_utf8(bytes).unwrap();
        assert!(json_str.contains("Eve"));
    }

    #[test]
    fn test_round_trip() {
        let original = TestStruct {
            name: "Frank".to_string(),
            age: 45,
            tags: vec!["senior".to_string(), "architect".to_string()],
        };

        let json = to_json(&original).unwrap();
        let decoded: TestStruct = from_json(&json).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_round_trip_bytes() {
        let original = TestStruct {
            name: "Grace".to_string(),
            age: 32,
            tags: vec!["data".to_string()],
        };

        let bytes = to_json_bytes(&original).unwrap();
        let decoded: TestStruct = from_json_bytes(&bytes).unwrap();

        assert_eq!(original, decoded);
    }
}
