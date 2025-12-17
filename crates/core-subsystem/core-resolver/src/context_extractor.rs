// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_trait::async_trait;
use core_model::{
    context_type::{
        ContextContainer, ContextField, ContextFieldType, ContextSelection,
        ContextSelectionElement, ContextType,
    },
    primitive_type::{self, NumberLiteral, PrimitiveType, PrimitiveValue},
    types::FieldType,
};
use futures::StreamExt;

use common::{
    context::{ContextExtractionError, RequestContext},
    value::{Val, val::ValNumber},
};

/// Extract context objects from the request context.
#[async_trait]
pub trait ContextExtractor {
    fn context_type(&self, context_type_name: &str) -> &ContextType;

    /// Extract the context object.
    ///
    /// If the context type is defined as:
    ///
    /// ```exo
    /// context AuthContext {
    ///   id: Int
    ///   name: String
    ///   role: String
    /// }
    /// ```
    ///
    /// Then calling this with `context_name` set to `"AuthContext"` will return an object
    /// such as:
    ///
    /// ```json
    /// {
    ///   id: 1,
    ///   name: "John",
    ///   role: "admin",
    /// }
    /// ```
    async fn extract_context(
        &self,
        request_context: &RequestContext,
        context_type_name: &str,
    ) -> Result<Option<Val>, ContextExtractionError> {
        let context_type = self.context_type(context_type_name);
        let field_values: HashMap<_, _> = futures::stream::iter(context_type.fields.iter())
            .then(|context_field| async {
                extract_context_field(request_context, context_type, context_field)
                    .await
                    .map(|value| value.map(|value| (context_field.name.clone(), value.clone())))
            })
            .collect::<Vec<Result<_, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        Ok(Some(Val::Object(field_values)))
    }

    /// Extract the context object selection.
    ///
    /// This method is similar to `extract_context` but it allows to select a specific field from
    /// the context object. For example, consider the context type and the context object in the
    /// documentation of [`extract_context`](Self::extract_context). Calling this method with
    /// `context_selection` set to
    /// `AccessContextSelection::Select(AccessContextSelection("AuthContext"), "role")` will return
    /// the value `"admin"`.
    async fn extract_context_selection<'a>(
        &self,
        request_context: &'a RequestContext<'a>,
        context_selection: &ContextSelection,
    ) -> Result<Option<&'a Val>, ContextExtractionError> {
        let context_type = self.context_type(&context_selection.context_name);

        let context_field = context_type
            .fields
            .iter()
            .find(|f| f.name == context_selection.path.0)
            .ok_or_else(|| {
                ContextExtractionError::FieldNotFound(context_selection.path.0.to_string())
            })?;

        let context_selection_path = &context_selection.path.1;

        let context_value =
            extract_context_field(request_context, context_type, context_field).await?;

        if context_selection_path.is_empty() {
            Ok(context_value)
        } else if context_selection_path.len() == 1 {
            match context_selection_path.first().unwrap() {
                ContextSelectionElement::NormalCall {
                    function_name,
                    args,
                } => {
                    if function_name == "contains" {
                        match context_value {
                            Some(context_value) => {
                                let search_value = args.first().unwrap();

                                match context_value {
                                    Val::List(values) => {
                                        let res = values.iter().any(|element| {
                                            match (element, search_value) {
                                                (
                                                    Val::String(s),
                                                    PrimitiveValue::String(search),
                                                ) => s == search,
                                                (
                                                    Val::Number(ValNumber::I64(i)),
                                                    PrimitiveValue::Number(NumberLiteral::Int(
                                                        search,
                                                    )),
                                                ) => *i == *search,
                                                (
                                                    Val::Number(ValNumber::F64(f)),
                                                    PrimitiveValue::Number(NumberLiteral::Float(
                                                        search,
                                                    )),
                                                ) => *f == *search,
                                                (Val::Bool(b), PrimitiveValue::Boolean(search)) => {
                                                    *b == *search
                                                }
                                                _ => false,
                                            }
                                        });

                                        Ok(Some(if res {
                                            &common::value::val::TRUE
                                        } else {
                                            &common::value::val::FALSE
                                        }))
                                    }
                                    _ => Err(ContextExtractionError::TypeMismatch {
                                        expected: "list".to_string(),
                                        actual: context_value.to_string(),
                                    }),
                                }
                            }
                            None => Ok(Some(&common::value::val::FALSE)),
                        }
                    } else {
                        Err(
                            ContextExtractionError::UnexpectedFunctionCallInContextSelection(
                                function_name.to_string(),
                            ),
                        )
                    }
                }
                _ => Err(ContextExtractionError::Generic(
                    "Unexpected context selection element".to_string(),
                )),
            }
        } else {
            Err(ContextExtractionError::Generic(
                "Unexpected context selection path".to_string(),
            ))
        }
    }
}

async fn extract_context_field<'a>(
    request_context: &'a RequestContext<'a>,
    context_type: &ContextType,
    context_field: &ContextField,
) -> Result<Option<&'a Val>, ContextExtractionError> {
    let typ = &context_field.typ;

    let coerce_fn = |value: Val| -> Result<Val, ContextExtractionError> { coerce(value, typ) };

    let raw_val = request_context
        .extract_context_field(
            &context_type.name,
            &context_field.source.annotation_name,
            &context_field.source.value.as_deref(),
            &context_field.name,
            &coerce_fn,
        )
        .await?;

    // If the field type is optional, we return Val::Null for an empty value.
    let option_sensitive_value = match typ {
        FieldType::Optional(_) => Some(raw_val.unwrap_or(&Val::Null)),
        _ => raw_val,
    };

    Ok(option_sensitive_value)
}

fn coerce(value: Val, typ: &ContextFieldType) -> Result<Val, ContextExtractionError> {
    match (value, typ) {
        (Val::List(elem), ContextFieldType::List(typ)) => {
            let coerced = elem
                .into_iter()
                .map(|elem| coerce(elem, typ))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Val::List(coerced))
        }
        (value, typ) => coerce_primitive(value, typ.innermost()),
    }
}

fn coerce_primitive(value: Val, typ: &PrimitiveType) -> Result<Val, ContextExtractionError> {
    // Special case for null values.
    // If the context value is null, we can return it as is for any type. This allows correct
    // handling of expressions such as `<something> || SomeContext.role == "admin"` when
    // `SomeContext.role` isn't supplied. In this case, the `SomeContext.role == "admin"` will
    // evaluate to `false`, and the `||` operator will return the value of `<something>`.
    if value == Val::Null {
        return Ok(value);
    }

    match typ {
        PrimitiveType::Plain(primitive_type) => {
            let type_name = primitive_type.name();

            if type_name == primitive_type::JsonType::NAME {
                return match value {
                    Val::String(_)
                    | Val::List(_)
                    | Val::Object(_)
                    | Val::Bool(_)
                    | Val::Number(_)
                    | Val::Null => Ok(value),
                    _ => Err(ContextExtractionError::TypeMismatch {
                        expected: typ.name(),
                        actual: value.to_string(),
                    }),
                };
            }

            match (&value, type_name) {
                // Direct matches for compatible types
                (Val::String(_), _type_name)
                    if primitive_type.name() == primitive_type::StringType::NAME =>
                {
                    Ok(value)
                }
                (Val::Number(_), _type_name)
                    if primitive_type.name() == primitive_type::IntType::NAME =>
                {
                    Ok(value)
                }
                (Val::Number(_), _type_name)
                    if primitive_type.name() == primitive_type::FloatType::NAME =>
                {
                    Ok(value)
                }
                (Val::Bool(_), _type_name)
                    if primitive_type.name() == primitive_type::BooleanType::NAME =>
                {
                    Ok(value)
                }
                (Val::String(_), _type_name)
                    if primitive_type.name() == primitive_type::UuidType::NAME =>
                {
                    Ok(value)
                }

                // String coercion to other types
                (Val::String(str), _type_name)
                    if primitive_type.name() == primitive_type::IntType::NAME =>
                {
                    str.parse::<i64>()
                        .map(|i| Val::Number(ValNumber::I64(i)))
                        .map_err(|_| ContextExtractionError::TypeMismatch {
                            expected: typ.name(),
                            actual: str.clone(),
                        })
                }
                (Val::String(str), _type_name)
                    if primitive_type.name() == primitive_type::FloatType::NAME =>
                {
                    str.parse::<f64>()
                        .map(|f| Val::Number(ValNumber::F64(f)))
                        .map_err(|_| ContextExtractionError::TypeMismatch {
                            expected: typ.name(),
                            actual: str.clone(),
                        })
                }
                (Val::String(str), _type_name)
                    if primitive_type.name() == primitive_type::BooleanType::NAME =>
                {
                    str.parse::<bool>().map(Val::Bool).map_err(|_| {
                        ContextExtractionError::TypeMismatch {
                            expected: typ.name(),
                            actual: str.clone(),
                        }
                    })
                }

                // Type mismatch
                _ => Err(ContextExtractionError::TypeMismatch {
                    expected: typ.name(),
                    actual: value.to_string(),
                }),
            }
        }
        PrimitiveType::Array(_) => Err(ContextExtractionError::TypeMismatch {
            expected: typ.name(),
            actual: value.to_string(),
        }),
    }
}

#[async_trait]
impl<T: ContextContainer + std::marker::Sync> ContextExtractor for T {
    fn context_type(&self, context_type_name: &str) -> &ContextType {
        let contexts = self.contexts();
        contexts.get_by_key(context_type_name).unwrap()
    }
}
