// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! RPC parameter validation: parse raw JSON params into typed `Val` values
//! while rejecting invalid input ("parse, don't validate").

use std::collections::HashMap;

use common::value::Val;
use common::value::val::ValNumber;
use thiserror::Error;

use crate::schema::{RpcComponents, RpcMethod, RpcTypeSchema};

/// Errors produced during RPC parameter validation.
#[derive(Error, Debug)]
pub enum RpcValidationError {
    #[error("Unknown parameter: {0}")]
    UnknownParameter(String),

    #[error("Missing required parameter: {0}")]
    MissingRequiredParameter(String),

    #[error("Type mismatch at '{path}': expected {expected}, got {got}")]
    TypeMismatch {
        expected: String,
        got: String,
        path: String,
    },

    #[error("Invalid enum value '{value}' at '{path}': allowed values are {allowed:?}")]
    InvalidEnumValue {
        value: String,
        allowed: Vec<String>,
        path: String,
    },

    #[error("{0}")]
    InvalidParams(String),
}

impl RpcValidationError {
    /// User-facing error message suitable for JSON-RPC error responses.
    pub fn user_message(&self) -> String {
        self.to_string()
    }
}

impl RpcMethod {
    /// Parse and validate raw JSON params against this method's parameter schema.
    ///
    /// Returns a map of parameter name → parsed `Val` for each recognized parameter.
    /// Rejects unknown parameters and missing required parameters.
    pub fn parse_params(
        &self,
        params: &Option<serde_json::Value>,
        components: &RpcComponents,
    ) -> Result<HashMap<String, Val>, RpcValidationError> {
        // No params defined and no params supplied → empty map
        if self.params.is_empty() {
            return match params {
                None | Some(serde_json::Value::Null) => Ok(HashMap::new()),
                Some(serde_json::Value::Object(obj)) if obj.is_empty() => Ok(HashMap::new()),
                Some(_) => Err(RpcValidationError::InvalidParams(
                    "This method takes no parameters".to_string(),
                )),
            };
        }

        // Build a mutable map of all supplied keys.
        // JSON-RPC treats `"params": null` the same as omitted params, so both
        // map to an empty set of supplied keys. Any required params will be
        // caught as missing in the loop below.
        let mut supplied: HashMap<String, serde_json::Value> = match params {
            None | Some(serde_json::Value::Null) => HashMap::new(),
            Some(serde_json::Value::Object(obj)) => {
                obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            }
            Some(_) => {
                return Err(RpcValidationError::InvalidParams(
                    "params must be an object".to_string(),
                ));
            }
        };

        let mut result = HashMap::new();

        // Iterate over defined parameters
        for param in &self.params {
            if let Some(json_value) = supplied.remove(&param.name) {
                let val = parse_value(&json_value, &param.schema, components, &param.name)?;
                result.insert(param.name.clone(), val);
            } else if param.is_required() {
                return Err(RpcValidationError::MissingRequiredParameter(
                    param.name.clone(),
                ));
            }
            // Optional and absent → skip
        }

        // Any remaining keys are unknown parameters
        if !supplied.is_empty() {
            let mut unknown_key = supplied.into_keys().collect::<Vec<_>>();
            unknown_key.sort();
            return Err(RpcValidationError::UnknownParameter(unknown_key.join(", ")));
        }

        Ok(result)
    }
}

/// Recursively parse a JSON value against an RPC type schema, producing a `Val`.
fn parse_value(
    json: &serde_json::Value,
    schema: &RpcTypeSchema,
    components: &RpcComponents,
    path: &str,
) -> Result<Val, RpcValidationError> {
    match schema {
        RpcTypeSchema::Optional { inner } => {
            if json.is_null() {
                Ok(Val::Null)
            } else {
                parse_value(json, inner, components, path)
            }
        }

        // TODO: Enforce `validation` constraints (e.g. IntConstraints min/max) here
        RpcTypeSchema::Scalar {
            type_name,
            validation: _,
        } => parse_scalar(json, type_name, path),

        RpcTypeSchema::Enum { values } => match json.as_str() {
            Some(s) => {
                if values.contains(&s.to_string()) {
                    Ok(Val::Enum(s.to_string()))
                } else {
                    Err(RpcValidationError::InvalidEnumValue {
                        value: s.to_string(),
                        allowed: values.clone(),
                        path: path.to_string(),
                    })
                }
            }
            None => Err(RpcValidationError::TypeMismatch {
                expected: "string (enum)".to_string(),
                got: json_type_name(json),
                path: path.to_string(),
            }),
        },

        RpcTypeSchema::Object { type_ref } => {
            let obj_type = components.get_schema(type_ref).ok_or_else(|| {
                RpcValidationError::InvalidParams(format!("Unknown type: {type_ref}"))
            })?;

            match json {
                serde_json::Value::Object(map) => {
                    let mut supplied: HashMap<String, &serde_json::Value> =
                        map.iter().map(|(k, v)| (k.clone(), v)).collect();

                    let mut result = HashMap::new();

                    for field in &obj_type.fields {
                        if let Some(field_json) = supplied.remove(&field.name) {
                            let field_path = format!("{path}.{}", field.name);
                            let val =
                                parse_value(field_json, &field.schema, components, &field_path)?;
                            result.insert(field.name.clone(), val);
                        }
                        // Fields in object types are typically optional (the schema marks them as Optional)
                        // Required-ness is enforced by the schema itself during recursion
                    }

                    // Unknown fields → error
                    if !supplied.is_empty() {
                        let mut unknown_keys = supplied.into_keys().collect::<Vec<_>>();
                        unknown_keys.sort();
                        return Err(RpcValidationError::InvalidParams(format!(
                            "Unknown field in {path}: {}",
                            unknown_keys.join(", ")
                        )));
                    }

                    Ok(Val::Object(result))
                }
                _ => Err(RpcValidationError::TypeMismatch {
                    expected: format!("object ({type_ref})"),
                    got: json_type_name(json),
                    path: path.to_string(),
                }),
            }
        }

        RpcTypeSchema::Array { items } => match json {
            serde_json::Value::Array(arr) => {
                let vals: Result<Vec<Val>, _> = arr
                    .iter()
                    .enumerate()
                    .map(|(i, elem)| {
                        let elem_path = format!("{path}[{i}]");
                        parse_value(elem, items, components, &elem_path)
                    })
                    .collect();
                Ok(Val::List(vals?))
            }
            _ => Err(RpcValidationError::TypeMismatch {
                expected: "array".to_string(),
                got: json_type_name(json),
                path: path.to_string(),
            }),
        },
    }
}

/// Parse a scalar JSON value into a `Val` based on the scalar type name.
fn parse_scalar(
    json: &serde_json::Value,
    type_name: &str,
    path: &str,
) -> Result<Val, RpcValidationError> {
    match type_name {
        "Int" => match json {
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Val::Number(ValNumber::I64(i)))
                } else {
                    Err(RpcValidationError::TypeMismatch {
                        expected: "integer".to_string(),
                        got: "float".to_string(),
                        path: path.to_string(),
                    })
                }
            }
            _ => Err(RpcValidationError::TypeMismatch {
                expected: "Int".to_string(),
                got: json_type_name(json),
                path: path.to_string(),
            }),
        },
        "Float" | "Decimal" => match json {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Val::Number(ValNumber::F64(f)))
                } else {
                    Err(RpcValidationError::TypeMismatch {
                        expected: type_name.to_string(),
                        got: "number".to_string(),
                        path: path.to_string(),
                    })
                }
            }
            _ => Err(RpcValidationError::TypeMismatch {
                expected: type_name.to_string(),
                got: json_type_name(json),
                path: path.to_string(),
            }),
        },
        "Boolean" => match json {
            serde_json::Value::Bool(b) => Ok(Val::Bool(*b)),
            _ => Err(RpcValidationError::TypeMismatch {
                expected: "Boolean".to_string(),
                got: json_type_name(json),
                path: path.to_string(),
            }),
        },
        // String-like scalars: String, Uuid, DateTime, LocalDateTime, LocalDate, LocalTime, etc.
        // Nulls should be handled by the Optional wrapper, not here.
        _ => match json {
            serde_json::Value::String(s) => Ok(Val::String(s.clone())),
            // Allow numbers for numeric-compatible scalar types (e.g. Bigint passed as number)
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Val::Number(ValNumber::I64(i)))
                } else if let Some(f) = n.as_f64() {
                    Ok(Val::Number(ValNumber::F64(f)))
                } else {
                    Ok(Val::String(n.to_string()))
                }
            }
            _ => Err(RpcValidationError::TypeMismatch {
                expected: type_name.to_string(),
                got: json_type_name(json),
                path: path.to_string(),
            }),
        },
    }
}

/// Return a human-readable name for a JSON value type.
fn json_type_name(json: &serde_json::Value) -> String {
    match json {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcTypeSchema};

    fn make_test_schema() -> (RpcMethod, RpcComponents) {
        let mut components = RpcComponents::new();
        components.schemas.insert(
            "TodoFilter".to_string(),
            RpcObjectType::new("TodoFilter")
                .with_field(RpcObjectField::new(
                    "title",
                    RpcTypeSchema::optional(RpcTypeSchema::scalar("String")),
                ))
                .with_field(RpcObjectField::new(
                    "completed",
                    RpcTypeSchema::optional(RpcTypeSchema::scalar("Boolean")),
                )),
        );

        let method = RpcMethod::new(
            "get_todos".to_string(),
            RpcTypeSchema::array(RpcTypeSchema::object("Todo")),
        )
        .with_param(RpcParameter::new(
            "where",
            RpcTypeSchema::optional(RpcTypeSchema::object("TodoFilter")),
        ))
        .with_param(RpcParameter::new(
            "limit",
            RpcTypeSchema::optional(RpcTypeSchema::scalar("Int")),
        ))
        .with_param(RpcParameter::new(
            "offset",
            RpcTypeSchema::optional(RpcTypeSchema::scalar("Int")),
        ));

        (method, components)
    }

    #[test]
    fn test_parse_params_valid() {
        let (method, components) = make_test_schema();
        let params = serde_json::json!({
            "where": {"title": "test"},
            "limit": 10
        });
        let result = method
            .parse_params(&Some(params), &components)
            .expect("should succeed");
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("where"));
        assert!(result.contains_key("limit"));
    }

    #[test]
    fn test_parse_params_no_params() {
        let (method, components) = make_test_schema();
        let result = method
            .parse_params(&None, &components)
            .expect("should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_params_unknown_parameter() {
        let (method, components) = make_test_schema();
        let params = serde_json::json!({
            "bogus": true
        });
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        assert!(
            err.to_string().contains("Unknown parameter: bogus"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_parse_params_unknown_field_in_object() {
        let (method, components) = make_test_schema();
        let params = serde_json::json!({
            "where": {"nonExistent": "value"}
        });
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        assert!(
            err.to_string().contains("Unknown field in where"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_parse_params_type_mismatch() {
        let (method, components) = make_test_schema();
        let params = serde_json::json!({
            "limit": "not a number"
        });
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        assert!(err.to_string().contains("Type mismatch"), "got: {}", err);
    }

    #[test]
    fn test_parse_params_missing_required() {
        let components = RpcComponents::new();
        let method = RpcMethod::new("get_item".to_string(), RpcTypeSchema::scalar("String"))
            .with_param(RpcParameter::new("id", RpcTypeSchema::scalar("Int")));

        let err = method.parse_params(&None, &components).unwrap_err();
        assert!(
            err.to_string().contains("Missing required parameter: id"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_parse_enum_value() {
        let components = RpcComponents::new();
        let method = RpcMethod::new("test".to_string(), RpcTypeSchema::scalar("String"))
            .with_param(RpcParameter::new(
                "direction",
                RpcTypeSchema::enum_type(vec!["ASC".to_string(), "DESC".to_string()]),
            ));

        let params = serde_json::json!({"direction": "ASC"});
        let result = method.parse_params(&Some(params), &components).unwrap();
        assert_eq!(result["direction"], Val::Enum("ASC".to_string()));

        let params = serde_json::json!({"direction": "INVALID"});
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        assert!(
            err.to_string().contains("Invalid enum value"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_parse_array() {
        let components = RpcComponents::new();
        let method =
            RpcMethod::new("test".to_string(), RpcTypeSchema::scalar("String")).with_param(
                RpcParameter::new("ids", RpcTypeSchema::array(RpcTypeSchema::scalar("Int"))),
            );

        let params = serde_json::json!({"ids": [1, 2, 3]});
        let result = method.parse_params(&Some(params), &components).unwrap();
        assert!(matches!(result["ids"], Val::List(_)));
    }

    #[test]
    fn test_null_for_required_scalar_is_rejected() {
        let components = RpcComponents::new();
        let method = RpcMethod::new("get_item".to_string(), RpcTypeSchema::scalar("String"))
            .with_param(RpcParameter::new("id", RpcTypeSchema::scalar("Int")));

        let params = serde_json::json!({"id": null});
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        assert!(err.to_string().contains("Type mismatch"), "got: {}", err);
    }

    #[test]
    fn test_null_for_required_string_scalar_is_rejected() {
        let components = RpcComponents::new();
        let method = RpcMethod::new("get_item".to_string(), RpcTypeSchema::scalar("String"))
            .with_param(RpcParameter::new("name", RpcTypeSchema::scalar("String")));

        let params = serde_json::json!({"name": null});
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        assert!(err.to_string().contains("Type mismatch"), "got: {}", err);
    }

    #[test]
    fn test_explicit_null_for_optional_param() {
        let (method, components) = make_test_schema();
        let params = serde_json::json!({"limit": null});
        let result = method.parse_params(&Some(params), &components).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["limit"], Val::Null);
    }

    #[test]
    fn test_multiple_unknown_params_deterministic() {
        let (method, components) = make_test_schema();
        let params = serde_json::json!({"zzz": 1, "aaa": 2});
        let err = method.parse_params(&Some(params), &components).unwrap_err();
        // Should list both unknown params in sorted order
        let msg = err.to_string();
        assert!(msg.contains("aaa"), "got: {}", msg);
        assert!(msg.contains("zzz"), "got: {}", msg);
        // "aaa" should come before "zzz"
        assert!(
            msg.find("aaa").unwrap() < msg.find("zzz").unwrap(),
            "got: {}",
            msg
        );
    }
}
