// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! RPC parameter validation using TypeValidation from core-model.
//!
//! This module provides validation logic that can be used both for
//! RPC parameter validation and, in the future, for GraphQL validation.

use core_model::types::{FloatConstraints, IntConstraints, StringConstraints, TypeValidation};
use serde_json::Value;
use std::fmt;

/// Errors that can occur during RPC parameter validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// A required parameter is missing
    MissingParameter { param: String },
    /// The parameter type doesn't match the expected type
    TypeMismatch {
        param: String,
        expected: &'static str,
        actual: String,
    },
    /// An integer value is out of the allowed range
    IntegerOutOfRange {
        param: String,
        value: i64,
        min: Option<i64>,
        max: Option<i64>,
    },
    /// A floating-point value is out of the allowed range
    NumberOutOfRange {
        param: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    /// A string is shorter than the minimum length
    StringTooShort {
        param: String,
        length: usize,
        min: usize,
    },
    /// A string is longer than the maximum length
    StringTooLong {
        param: String,
        length: usize,
        max: usize,
    },
    /// A value is not one of the allowed enum values
    InvalidEnumValue {
        param: String,
        value: String,
        allowed: Vec<String>,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::MissingParameter { param } => {
                write!(f, "Missing required parameter: {}", param)
            }
            ValidationError::TypeMismatch {
                param,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Type mismatch for parameter '{}': expected {}, got {}",
                    param, expected, actual
                )
            }
            ValidationError::IntegerOutOfRange {
                param,
                value,
                min,
                max,
            } => {
                let range_desc = match (min, max) {
                    (Some(min), Some(max)) => format!("between {} and {}", min, max),
                    (Some(min), None) => format!("at least {}", min),
                    (None, Some(max)) => format!("at most {}", max),
                    (None, None) => "any value".to_string(),
                };
                write!(
                    f,
                    "Integer parameter '{}' value {} is out of range (must be {})",
                    param, value, range_desc
                )
            }
            ValidationError::NumberOutOfRange {
                param,
                value,
                min,
                max,
            } => {
                let range_desc = match (min, max) {
                    (Some(min), Some(max)) => format!("between {} and {}", min, max),
                    (Some(min), None) => format!("at least {}", min),
                    (None, Some(max)) => format!("at most {}", max),
                    (None, None) => "any value".to_string(),
                };
                write!(
                    f,
                    "Number parameter '{}' value {} is out of range (must be {})",
                    param, value, range_desc
                )
            }
            ValidationError::StringTooShort { param, length, min } => {
                write!(
                    f,
                    "String parameter '{}' is too short (length {} < minimum {})",
                    param, length, min
                )
            }
            ValidationError::StringTooLong { param, length, max } => {
                write!(
                    f,
                    "String parameter '{}' is too long (length {} > maximum {})",
                    param, length, max
                )
            }
            ValidationError::InvalidEnumValue {
                param,
                value,
                allowed,
            } => {
                write!(
                    f,
                    "Invalid value '{}' for parameter '{}' (allowed: {})",
                    value,
                    param,
                    allowed.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a value against TypeValidation constraints.
///
/// # Arguments
/// * `validation` - The validation constraints to apply
/// * `value` - The JSON value to validate
/// * `param_name` - The name of the parameter (for error messages)
///
/// # Returns
/// A vector of validation errors (empty if validation passes)
pub fn validate_with_constraints(
    validation: &TypeValidation,
    value: &Value,
    param_name: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match (validation, value) {
        (TypeValidation::Int(constraints), Value::Number(n)) => {
            if let Some(int_val) = n.as_i64() {
                errors.extend(validate_int(int_val, constraints, param_name));
            } else if let Some(float_val) = n.as_f64() {
                // Number doesn't fit in i64, try to validate as float
                if float_val.fract() != 0.0 {
                    errors.push(ValidationError::TypeMismatch {
                        param: param_name.to_string(),
                        expected: "integer",
                        actual: "float".to_string(),
                    });
                } else {
                    // It's a whole number but too large for i64
                    errors.push(ValidationError::IntegerOutOfRange {
                        param: param_name.to_string(),
                        value: float_val as i64, // This will be incorrect for very large values
                        min: constraints.min,
                        max: constraints.max,
                    });
                }
            }
        }
        (TypeValidation::Int(_), _) => {
            errors.push(ValidationError::TypeMismatch {
                param: param_name.to_string(),
                expected: "integer",
                actual: json_type_name(value).to_string(),
            });
        }
        (TypeValidation::Float(constraints), Value::Number(n)) => {
            if let Some(float_val) = n.as_f64() {
                errors.extend(validate_float(float_val, constraints, param_name));
            }
        }
        (TypeValidation::Float(_), _) => {
            errors.push(ValidationError::TypeMismatch {
                param: param_name.to_string(),
                expected: "number",
                actual: json_type_name(value).to_string(),
            });
        }
        (TypeValidation::String(constraints), Value::String(s)) => {
            errors.extend(validate_string(s, constraints, param_name));
        }
        (TypeValidation::String(_), _) => {
            errors.push(ValidationError::TypeMismatch {
                param: param_name.to_string(),
                expected: "string",
                actual: json_type_name(value).to_string(),
            });
        }
    }

    errors
}

/// Validate an integer value against constraints.
fn validate_int(
    value: i64,
    constraints: &IntConstraints,
    param_name: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if let Some(min) = constraints.min
        && value < min
    {
        errors.push(ValidationError::IntegerOutOfRange {
            param: param_name.to_string(),
            value,
            min: Some(min),
            max: constraints.max,
        });
    }

    if let Some(max) = constraints.max
        && value > max
    {
        errors.push(ValidationError::IntegerOutOfRange {
            param: param_name.to_string(),
            value,
            min: constraints.min,
            max: Some(max),
        });
    }

    errors
}

/// Validate a floating-point value against constraints.
fn validate_float(
    value: f64,
    constraints: &FloatConstraints,
    param_name: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if let Some(min) = constraints.min
        && value < min
    {
        errors.push(ValidationError::NumberOutOfRange {
            param: param_name.to_string(),
            value,
            min: Some(min),
            max: constraints.max,
        });
    }

    if let Some(max) = constraints.max
        && value > max
    {
        errors.push(ValidationError::NumberOutOfRange {
            param: param_name.to_string(),
            value,
            min: constraints.min,
            max: Some(max),
        });
    }

    errors
}

/// Validate a string value against constraints.
fn validate_string(
    value: &str,
    constraints: &StringConstraints,
    param_name: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let length = value.chars().count();

    if let Some(min_length) = constraints.min_length
        && length < min_length
    {
        errors.push(ValidationError::StringTooShort {
            param: param_name.to_string(),
            length,
            min: min_length,
        });
    }

    if let Some(max_length) = constraints.max_length
        && length > max_length
    {
        errors.push(ValidationError::StringTooLong {
            param: param_name.to_string(),
            length,
            max: max_length,
        });
    }

    errors
}

/// Get the JSON type name for a value.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Validate that a value is one of the allowed enum values.
pub fn validate_enum(value: &str, allowed: &[String], param_name: &str) -> Vec<ValidationError> {
    if allowed.iter().any(|v| v == value) {
        Vec::new()
    } else {
        vec![ValidationError::InvalidEnumValue {
            param: param_name.to_string(),
            value: value.to_string(),
            allowed: allowed.to_vec(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_int_within_range() {
        let constraints = IntConstraints::from_range(1, 100);
        let value = serde_json::json!(50);
        let errors = validate_with_constraints(&TypeValidation::Int(constraints), &value, "test");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_int_below_min() {
        let constraints = IntConstraints::from_range(1, 100);
        let value = serde_json::json!(0);
        let errors = validate_with_constraints(&TypeValidation::Int(constraints), &value, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            ValidationError::IntegerOutOfRange { .. }
        ));
    }

    #[test]
    fn test_validate_int_above_max() {
        let constraints = IntConstraints::from_range(1, 100);
        let value = serde_json::json!(150);
        let errors = validate_with_constraints(&TypeValidation::Int(constraints), &value, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            ValidationError::IntegerOutOfRange { .. }
        ));
    }

    #[test]
    fn test_validate_int_type_mismatch() {
        let constraints = IntConstraints::from_range(1, 100);
        let value = serde_json::json!("not a number");
        let errors = validate_with_constraints(&TypeValidation::Int(constraints), &value, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn test_validate_float_within_range() {
        let constraints = FloatConstraints::from_range(0.0, 1.0);
        let value = serde_json::json!(0.5);
        let errors = validate_with_constraints(&TypeValidation::Float(constraints), &value, "test");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_float_below_min() {
        let constraints = FloatConstraints::from_range(0.0, 1.0);
        let value = serde_json::json!(-0.5);
        let errors = validate_with_constraints(&TypeValidation::Float(constraints), &value, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            ValidationError::NumberOutOfRange { .. }
        ));
    }

    #[test]
    fn test_validate_string_within_length() {
        let constraints = StringConstraints::new(Some(1), Some(10));
        let value = serde_json::json!("hello");
        let errors =
            validate_with_constraints(&TypeValidation::String(constraints), &value, "test");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_string_too_short() {
        let constraints = StringConstraints::new(Some(5), Some(10));
        let value = serde_json::json!("hi");
        let errors =
            validate_with_constraints(&TypeValidation::String(constraints), &value, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::StringTooShort { .. }));
    }

    #[test]
    fn test_validate_string_too_long() {
        let constraints = StringConstraints::with_max_length(5);
        let value = serde_json::json!("hello world");
        let errors =
            validate_with_constraints(&TypeValidation::String(constraints), &value, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::StringTooLong { .. }));
    }

    #[test]
    fn test_validate_enum_valid() {
        let allowed = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let errors = validate_enum("B", &allowed, "test");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_enum_invalid() {
        let allowed = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let errors = validate_enum("D", &allowed, "test");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            ValidationError::InvalidEnumValue { .. }
        ));
    }

    #[test]
    fn test_error_display() {
        let error = ValidationError::IntegerOutOfRange {
            param: "age".to_string(),
            value: 150,
            min: Some(0),
            max: Some(120),
        };
        assert_eq!(
            error.to_string(),
            "Integer parameter 'age' value 150 is out of range (must be between 0 and 120)"
        );
    }
}
