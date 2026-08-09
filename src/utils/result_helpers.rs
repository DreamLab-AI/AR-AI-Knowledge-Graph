//! Safe error handling utilities to eliminate unsafe .unwrap() calls
//!
//! This module provides utilities to replace 432 unsafe .unwrap() calls and
//! standardize 1,544 error handling patterns across the codebase.
//!
//! ## Safety Philosophy
//! - Production code should NEVER panic
//! - All errors should provide meaningful context
//! - Logging is better than crashing
//! - Defaults are better than panics

use crate::errors::{VisionClawError, VisionClawResult};
use tracing::{error, warn};

/// Safely converts f64 to serde_json::Number, replacing NaN/Infinity with 0.0
/// This is a common pattern when building JSON responses with numeric data.
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::safe_json_number;
/// let num = safe_json_number(42.5);
/// assert!(num.is_some());
/// let nan_num = safe_json_number(f64::NAN);
/// assert_eq!(nan_num, serde_json::Number::from_f64(0.0));
/// ```
pub fn safe_json_number(value: f64) -> serde_json::Number {
    use serde_json::Number;

    // SAFETY: 0.0 is always a valid finite f64, so from_f64 never returns None for it
    let zero = Number::from_f64(0.0).expect("0.0 is always a valid JSON number");

    if value.is_finite() {
        Number::from_f64(value).unwrap_or(zero)
    } else {
        // NaN or Infinity - replace with 0.0
        warn!(
            "safe_json_number: Replacing non-finite value ({}) with 0.0",
            value
        );
        zero
    }
}

/// Safely unwraps an Option with logging instead of panic.
/// Unlike `.unwrap()` which panics, this function logs a warning and returns
/// a default value when the Option is None.
/// # Arguments
/// * `option` - The Option to unwrap
/// * `default` - The default value to return if None
/// * `context` - Context message for logging
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::safe_unwrap;
/// let value = Some(42);
/// let result = safe_unwrap(value, 0, "getting user count");
/// assert_eq!(result, 42);
/// let empty: Option<i32> = None;
/// let result = safe_unwrap(empty, 0, "getting missing value");
/// assert_eq!(result, 0); // Returns default, doesn't panic
/// ```
pub fn safe_unwrap<T>(option: Option<T>, default: T, context: &str) -> T {
    match option {
        Some(value) => value,
        None => {
            warn!("safe_unwrap: Using default value for {}", context);
            default
        }
    }
}

/// Safely unwraps an Option or returns an error with context.
/// Converts Option<T> to Result<T, VisionClawError> with meaningful error message.
/// # Arguments
/// * `option` - The Option to unwrap
/// * `context` - Error message if None
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::ok_or_error;
/// let value = Some(42);
/// let result = ok_or_error(value, "Failed to get user ID");
/// assert!(result.is_ok());
/// let empty: Option<i32> = None;
/// let result = ok_or_error(empty, "Failed to get user ID");
/// assert!(result.is_err());
/// ```
pub fn ok_or_error<T>(option: Option<T>, context: &str) -> VisionClawResult<T> {
    option.ok_or_else(|| VisionClawError::Generic {
        message: context.to_string(),
        source: None,
    })
}

/// Adds context to any error type and converts to VisionClawError.
/// Replaces manual `.map_err(|e| format!("context: {}", e))` patterns.
/// # Arguments
/// * `result` - The Result to add context to
/// * `context` - Context message to prepend
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::map_err_context;
/// use std::fs::File;
/// let result = File::open("nonexistent.txt");
/// let with_context = map_err_context(result, "Failed to open config file");
/// // Error now includes: "Failed to open config file: No such file or directory"
/// ```
pub fn map_err_context<T, E>(result: Result<T, E>, context: &str) -> VisionClawResult<T>
where
    E: std::error::Error + Send + Sync + 'static,
{
    result.map_err(|e| VisionClawError::Generic {
        message: format!("{}: {}", context, e),
        source: Some(std::sync::Arc::new(e)),
    })
}

/// Converts any Result to VisionClawResult with context.
/// More flexible than map_err_context, works with any error type.
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::to_vf_error;
/// fn parse_config() -> Result<i32, String> {
///     Err("Invalid format".to_string())
/// }
/// let result = to_vf_error(parse_config(), "Failed to parse config");
/// assert!(result.is_err());
/// ```
pub fn to_vf_error<T, E>(result: Result<T, E>, context: &str) -> VisionClawResult<T>
where
    E: std::fmt::Display,
{
    result.map_err(|e| VisionClawError::Generic {
        message: format!("{}: {}", context, e),
        source: None,
    })
}

/// Unwraps Option with logging, returns None if empty.
/// Logs a warning but doesn't panic, allows graceful degradation.
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::ok_or_log;
/// let value = Some(42);
/// assert_eq!(ok_or_log(value, "test value"), Some(42));
/// let empty: Option<i32> = None;
/// assert_eq!(ok_or_log(empty, "missing value"), None); // Logs warning
/// ```
pub fn ok_or_log<T>(option: Option<T>, message: &str) -> Option<T> {
    if option.is_none() {
        warn!("ok_or_log: {}", message);
    }
    option
}

/// Unwraps Option or returns default with logging.
/// Safer alternative to `.unwrap_or_default()` with visibility into when defaults are used.
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::unwrap_or_default_log;
/// let value = Some(42);
/// assert_eq!(unwrap_or_default_log(value, "test"), 42);
/// let empty: Option<i32> = None;
/// assert_eq!(unwrap_or_default_log(empty, "missing"), 0); // Logs warning
/// ```
pub fn unwrap_or_default_log<T: Default>(option: Option<T>, message: &str) -> T {
    match option {
        Some(value) => value,
        None => {
            warn!("unwrap_or_default_log: Using default for {}", message);
            T::default()
        }
    }
}

/// Safely unwraps Result or logs error and returns default.
/// For cases where we want to continue with a default instead of propagating errors.
/// # Examples
/// ```rust,ignore
/// use visionclaw::utils::result_helpers::result_or_default_log;
/// fn may_fail() -> Result<i32, String> {
///     Err("oops".to_string())
/// }
/// let result = result_or_default_log(may_fail(), 0, "operation");
/// assert_eq!(result, 0); // Returns default, logs error
/// ```
pub fn result_or_default_log<T: Default, E: std::fmt::Display>(
    result: Result<T, E>,
    default: T,
    context: &str,
) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            error!("result_or_default_log: {} failed: {}", context, e);
            default
        }
    }
}

/// Macro for adding context to Results with ? operator support.
/// Replaces manual error handling with clean, context-rich errors.
/// # Examples
/// ```rust,ignore
/// use visionclaw::try_with_context;
/// use visionclaw::errors::VisionClawResult;
/// use std::fs::File;
/// fn load_config() -> VisionClawResult<String> {
///     let file = try_with_context!(
///         File::open("config.json"),
///         "Failed to open config file"
///     );
///     // ... rest of function
///     Ok("config".to_string())
/// }
/// ```
#[macro_export]
macro_rules! try_with_context {
    ($expr:expr, $context:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                return Err($crate::errors::VisionClawError::Generic {
                    message: format!("{}: {}", $context, e),
                    source: Some(std::sync::Arc::new(e)),
                });
            }
        }
    };
}

/// Macro for safe unwrap with default value.
/// Cleaner syntax for unwrap_or_default_log.
/// # Examples
/// ```rust,ignore
/// use visionclaw::unwrap_or_default;
/// let value: Option<i32> = None;
/// let result = unwrap_or_default!(value, "user count");
/// assert_eq!(result, 0);
/// ```
#[macro_export]
macro_rules! unwrap_or_default {
    ($expr:expr, $context:expr) => {
        $crate::utils::result_helpers::unwrap_or_default_log($expr, $context)
    };
}

/// Macro for safe unwrap with custom default.
/// # Examples
/// ```rust,ignore
/// use visionclaw::safe_unwrap;
/// let value: Option<i32> = None;
/// let result = safe_unwrap!(value, 42, "default value");
/// assert_eq!(result, 42);
/// ```
#[macro_export]
macro_rules! safe_unwrap {
    ($expr:expr, $default:expr, $context:expr) => {
        $crate::utils::result_helpers::safe_unwrap($expr, $default, $context)
    };
}

/// Extension trait for Results to add VisionClaw-specific error handling.
pub trait ResultExt<T, E> {
    /// Add context to error with format string support.
    fn context(self, context: &str) -> VisionClawResult<T>;

    /// Add context with lazy evaluation (for expensive string construction).
    fn with_context<F>(self, f: F) -> VisionClawResult<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context(self, context: &str) -> VisionClawResult<T> {
        self.map_err(|e| VisionClawError::Generic {
            message: format!("{}: {}", context, e),
            source: Some(std::sync::Arc::new(e)),
        })
    }

    fn with_context<F>(self, f: F) -> VisionClawResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| VisionClawError::Generic {
            message: format!("{}: {}", f(), e),
            source: Some(std::sync::Arc::new(e)),
        })
    }
}

/// Extension trait for Options to add VisionClaw-specific handling.
pub trait OptionExt<T> {
    /// Convert Option to Result with context.
    fn context(self, context: &str) -> VisionClawResult<T>;

    /// Convert with lazy context evaluation.
    fn with_context<F>(self, f: F) -> VisionClawResult<T>
    where
        F: FnOnce() -> String;
}

impl<T> OptionExt<T> for Option<T> {
    fn context(self, context: &str) -> VisionClawResult<T> {
        self.ok_or_else(|| VisionClawError::Generic {
            message: context.to_string(),
            source: None,
        })
    }

    fn with_context<F>(self, f: F) -> VisionClawResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| VisionClawError::Generic {
            message: f(),
            source: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_unwrap_with_value() {
        let value = Some(42);
        let result = safe_unwrap(value, 0, "test value");
        assert_eq!(result, 42);
    }

    #[test]
    fn test_safe_unwrap_with_none() {
        let value: Option<i32> = None;
        let result = safe_unwrap(value, 99, "missing value");
        assert_eq!(result, 99);
    }

    #[test]
    fn test_ok_or_error_with_value() {
        let value = Some("test");
        let result = ok_or_error(value, "error context");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test");
    }

    #[test]
    fn test_ok_or_error_with_none() {
        let value: Option<&str> = None;
        let result = ok_or_error(value, "error context");
        assert!(result.is_err());
        if let Err(VisionClawError::Generic { message, .. }) = result {
            assert_eq!(message, "error context");
        } else {
            panic!("Expected Generic error");
        }
    }

    #[test]
    fn test_map_err_context() {
        use std::io::{Error, ErrorKind};

        let result: Result<(), Error> = Err(Error::new(ErrorKind::NotFound, "file missing"));
        let with_context = map_err_context(result, "Failed to read config");

        assert!(with_context.is_err());
        if let Err(VisionClawError::Generic { message, .. }) = with_context {
            assert!(message.contains("Failed to read config"));
            assert!(message.contains("file missing"));
        } else {
            panic!("Expected Generic error");
        }
    }

    #[test]
    fn test_to_vf_error() {
        let result: Result<i32, String> = Err("parse error".to_string());
        let vf_result = to_vf_error(result, "Failed to parse");

        assert!(vf_result.is_err());
        if let Err(VisionClawError::Generic { message, .. }) = vf_result {
            assert!(message.contains("Failed to parse"));
            assert!(message.contains("parse error"));
        } else {
            panic!("Expected Generic error");
        }
    }

    #[test]
    fn test_unwrap_or_default_log() {
        let value: Option<i32> = None;
        let result = unwrap_or_default_log(value, "test");
        assert_eq!(result, 0); // i32 default
    }

    #[test]
    fn test_result_ext_context() {
        use std::io::{Error, ErrorKind};

        let result: Result<(), Error> = Err(Error::new(ErrorKind::NotFound, "test"));
        let with_context = result.context("Operation failed");

        assert!(with_context.is_err());
    }

    #[test]
    fn test_option_ext_context() {
        let value: Option<i32> = None;
        let result = value.context("Value not found");

        assert!(result.is_err());
        if let Err(VisionClawError::Generic { message, .. }) = result {
            assert_eq!(message, "Value not found");
        }
    }

    #[test]
    fn test_result_or_default_log() {
        let result: Result<i32, &str> = Err("error");
        let value = result_or_default_log(result, 42, "test operation");
        assert_eq!(value, 42);
    }
}
