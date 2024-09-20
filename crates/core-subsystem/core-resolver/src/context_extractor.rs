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
    primitive_type::{PrimitiveType, PrimitiveValue},
    types::FieldType,
};
use futures::StreamExt;

use crate::{
    context::{ContextExtractionError, RequestContext},
    value::Val,
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
                                                (Val::Number(n), PrimitiveValue::Int(search)) => {
                                                    n.as_i64().unwrap() == *search
                                                }
                                                (Val::Bool(b), PrimitiveValue::Boolean(search)) => {
                                                    *b == *search
                                                }
                                                _ => false,
                                            }
                                        });

                                        Ok(Some(if res {
                                            &crate::value::val::TRUE
                                        } else {
                                            &crate::value::val::FALSE
                                        }))
                                    }
                                    _ => Err(ContextExtractionError::TypeMismatch {
                                        expected: "list".to_string(),
                                        actual: context_value.to_string(),
                                    }),
                                }
                            }
                            None => Ok(Some(&crate::value::val::FALSE)),
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
    match (value, typ) {
        // Special case for null values.
        // If the context value is null, we can return it as is for any type. This allows correct
        // handling of expressions such as `<something> || SomeContext.role == "admin"` when
        // `SomeContext.role` isn't supplied. In this case, the `SomeContext.role == "admin"` will
        // evaluate to `false`, and the `||` operator will return the value of `<something>`.
        (value, _) if value == Val::Null => Ok(value),
        (value @ Val::String(_), PrimitiveType::String) => Ok(value),
        (value @ Val::Number(_), PrimitiveType::Int) => Ok(value),
        (value @ Val::Number(_), PrimitiveType::Float) => Ok(value),
        (value @ Val::Bool(_), PrimitiveType::Boolean) => Ok(value),
        (value @ Val::String(_), PrimitiveType::Uuid) => Ok(value),
        (Val::String(str), pt) => match pt {
            PrimitiveType::Int => str
                .parse::<i64>()
                .map(|i| Val::Number(i.into()))
                .map_err(|_| ContextExtractionError::TypeMismatch {
                    expected: typ.name(),
                    actual: str,
                }),
            PrimitiveType::Float => str
                .parse::<f64>()
                .map(|f| Val::Number(serde_json::Number::from_f64(f).unwrap()))
                .map_err(|_| ContextExtractionError::TypeMismatch {
                    expected: typ.name(),
                    actual: str,
                }),
            PrimitiveType::Boolean => str.parse::<bool>().map(Val::Bool).map_err(|_| {
                ContextExtractionError::TypeMismatch {
                    expected: typ.name(),
                    actual: str,
                }
            }),
            _ => Err(ContextExtractionError::TypeMismatch {
                expected: typ.name(),
                actual: str,
            }),
        },
        (value, _) => Err(ContextExtractionError::TypeMismatch {
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
