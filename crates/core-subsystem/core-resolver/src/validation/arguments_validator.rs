// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_graphql_parser::{
    Pos, Positioned,
    types::{BaseType, ConstDirective, Field, InputValueDefinition, TypeKind},
};
use async_graphql_value::{ConstValue, Name, Number, Value, indexmap::IndexMap};
use bytes::Bytes;
use core_model::primitive_type;

use crate::{
    introspection::definition::schema::Schema, validation::validation_error::ValidationError,
};
use common::value::{UnresolvedVal, Val};

use super::underlying_type;

pub struct ArgumentValidator<'a> {
    schema: &'a Schema,
    variables: &'a HashMap<Name, ConstValue>,
    field: &'a Positioned<Field>,
}

impl<'a> ArgumentValidator<'a> {
    #[must_use]
    pub fn new(
        schema: &'a Schema,
        variables: &'a HashMap<Name, ConstValue>,
        field: &'a Positioned<Field>,
    ) -> Self {
        Self {
            schema,
            variables,
            field,
        }
    }

    /// Validations performed:
    /// - Ensure that all required arguments are provided
    /// - Ensure that there are no stray arguments (arguments that are not defined in the field)
    pub(super) fn validate(
        &self,
        field_argument_definition: &[&InputValueDefinition],
    ) -> Result<IndexMap<String, Val>, ValidationError> {
        // TODO: Ensure that the argument type is compatible with the argument definition
        // (currently, we do a partial check: object-shape, matched scalar, but no checks such
        // as a LocalTime argument is valid or the numbers fit the expected range).
        self.validate_arguments(field_argument_definition, &self.field.node.arguments)
    }

    fn validate_arguments(
        &self,
        field_argument_definitions: &[&InputValueDefinition],
        field_arguments: &[(Positioned<Name>, Positioned<Value>)],
    ) -> Result<IndexMap<String, Val>, ValidationError> {
        let field_name = self.field.node.name.node.as_str();

        // Stray arguments tracking: 1. Maintain a hashmap of all the arguments supplied in the query
        let mut field_arguments: IndexMap<_, _> = field_arguments
            .iter()
            .filter_map(|(name, value)| {
                // A few typical usages of GraphQL operations involve taking an
                // old value (typically has the `__typename` attribute while
                // querying--often added by clients such as Apollo for caching
                // purposes), update that value, and then send it as an argument
                // for an update mutation. To support such cases we will not
                // consider the __typename argument as a stray argument.
                if name.node == "__typename" {
                    None
                } else {
                    Some((&name.node, value))
                }
            })
            .collect();

        let validated_arguments = field_argument_definitions
            .iter()
            .filter_map(|argument_definition| {
                let argument_name = &argument_definition.name.node;
                // Stray arguments tracking: 2. Remove the argument being processed
                let argument_value = field_arguments.shift_remove(argument_name);

                self.validate_argument(argument_definition, argument_value)
                    .map(|argument_value| {
                        argument_value
                            .map(|argument_value| (argument_name.to_string(), argument_value))
                    })
            })
            .collect::<Result<_, _>>()?;

        // Stray arguments tracking: 3. If any arguments are left in the hashmap, they are stray arguments (not defined in the field)
        if !field_arguments.is_empty() {
            let stray_arguments = field_arguments
                .keys()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();

            Err(ValidationError::StrayArguments(
                stray_arguments,
                field_name.to_string(),
                self.field.pos,
            ))
        } else {
            Ok(validated_arguments)
        }
    }

    /// Validate a single argument
    /// Validations performed:
    /// - If the argument is a variable, the variable is defined
    /// - A null value is specified only for a nullable argument
    /// - Scalers match the expected type (but no deep checks such as a LocalTime argument is valid or the numbers fit the expected range).
    /// - Objects match the expected shape (recursively)
    /// - Lists match the expected shape
    fn validate_argument(
        &self,
        argument_definition: &InputValueDefinition,
        argument_value: Option<&Positioned<Value>>,
    ) -> Option<Result<Val, ValidationError>> {
        match argument_value {
            Some(value) => match &value.node {
                Value::Variable(name) => {
                    let resolved_variable = self.variables.get(name);
                    match resolved_variable {
                        Some(resolved_variable) => self.validate_argument(
                            argument_definition,
                            Some(&Positioned::new(
                                resolved_variable.clone().into_value(),
                                value.pos,
                            )),
                        ),
                        None => Some(Err(ValidationError::VariableNotFound(
                            name.to_string(),
                            self.field.pos,
                        ))),
                    }
                }
                Value::Null => Some(self.validate_null_argument(argument_definition, value.pos)),
                Value::Number(number) => {
                    Some(self.validate_number_argument(argument_definition, number, value.pos))
                }
                Value::String(string) => {
                    Some(self.validate_string_argument(argument_definition, string, value.pos))
                }
                Value::Boolean(boolean) => {
                    Some(self.validate_boolean_argument(argument_definition, boolean, value.pos))
                }
                Value::Binary(binary) => {
                    Some(self.validate_binary_argument(argument_definition, binary, value.pos))
                }
                Value::Enum(e) => Some(Ok(Val::Enum(e.to_string()))),
                Value::List(elems) => {
                    Some(self.validate_list_argument(argument_definition, elems, value.pos))
                }
                Value::Object(object) => {
                    Some(self.validate_object_argument(argument_definition, object, value.pos))
                }
            },
            None => {
                if argument_definition.ty.node.nullable {
                    None
                } else {
                    Some(Err(ValidationError::RequiredArgumentNotFound(
                        argument_definition.name.node.to_string(),
                        self.field.pos,
                    )))
                }
            }
        }
    }

    fn validate_null_argument(
        &self,
        argument_definition: &InputValueDefinition,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        let ty = &argument_definition.ty.node;

        if ty.nullable {
            Ok(Val::Null)
        } else {
            Err(ValidationError::RequiredArgumentNotFound(
                argument_definition.name.node.to_string(),
                pos,
            ))
        }
    }

    fn validate_number_argument(
        &self,
        argument_definition: &InputValueDefinition,
        number: &Number,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        let directives = argument_definition.directives.clone();

        if !directives.is_empty() {
            if let Some(value) = number.clone().as_i64() {
                validate_int_range(directives, value, pos)?
            } else if let Some(value) = number.clone().as_f64() {
                validate_float_range(directives, value, pos)?
            }
        }

        // TODO: Use the types from PrimitiveType (but that is currently in the builder crate, which we don't want to depend on)
        self.validate_scalar_argument(
            "Number",
            &[
                primitive_type::IntType::NAME,
                primitive_type::FloatType::NAME,
            ],
            || {
                Ok(Val::Number(number.clone().try_into().map_err(|_| {
                    ValidationError::InvalidArgumentType {
                        argument_name: argument_definition.name.node.to_string(),
                        expected_type: "Number".to_string(),
                        actual_type: "Number".to_string(),
                        pos,
                    }
                })?))
            },
            argument_definition,
            pos,
        )
    }

    fn validate_boolean_argument(
        &self,
        argument_definition: &InputValueDefinition,
        boolean: &bool,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        // TODO: Use the types from PrimitiveType (but that is currently in the builder crate, which we don't want to depend on)
        self.validate_scalar_argument(
            "Boolean",
            &["Boolean"],
            || Ok(Val::Bool(*boolean)),
            argument_definition,
            pos,
        )
    }

    fn validate_string_argument(
        &self,
        argument_definition: &InputValueDefinition,
        string: &str,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        // TODO: Use the types from PrimitiveType (but that is currently in the builder crate, which we don't want to depend on)
        self.validate_scalar_argument(
            "String",
            &[
                "String",
                "Decimal",
                "LocalDate",
                "LocalTime",
                "LocalDateTime",
                "Instant",
                "Uuid",
                "Vector",
                "Blob",
                "Json",
            ],
            || Ok(Val::String(string.to_string())),
            argument_definition,
            pos,
        )
    }

    fn validate_binary_argument(
        &self,
        argument_definition: &InputValueDefinition,
        bytes: &Bytes,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        self.validate_scalar_argument(
            "Binary",
            &["Binary"],
            || Ok(Val::Binary(bytes.clone())),
            argument_definition,
            pos,
        )
    }

    /// Validate a scalar argument
    /// Check if the argument is compatible with one the expected destination types
    fn validate_scalar_argument<const N: usize>(
        &self,
        argument_typename: &str,
        acceptable_destination_types: &[&str; N],
        to_val: impl FnOnce() -> Result<Val, ValidationError>,
        argument_definition: &InputValueDefinition,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        let ty = &argument_definition.ty.node;
        let underlying = underlying_type(ty);

        if acceptable_destination_types.contains(&underlying.as_str()) {
            to_val()
        } else {
            Err(ValidationError::InvalidArgumentType {
                argument_name: argument_definition.name.node.to_string(),
                expected_type: underlying.to_string(),
                actual_type: argument_typename.to_string(),
                pos,
            })
        }
    }

    /// Recursively validate an object argument
    fn validate_object_argument(
        &self,
        argument_definition: &InputValueDefinition,
        entires: &IndexMap<Name, Value>,
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        let ty = &argument_definition.ty.node;
        let underlying = underlying_type(ty);

        if underlying.as_str() == "Json" {
            let unresolved_value: UnresolvedVal = Value::Object(entires.clone()).into();
            let resolved_value = unresolved_value.resolve(&|name: &str| {
                self.variables
                    .get(&Name::new(name))
                    .cloned()
                    .ok_or_else(|| {
                        ValidationError::VariableNotFound(name.to_string(), Pos::default())
                    })
            });

            return resolved_value;
        }

        // We don't validate if the expected type is an object (and not a list), since the GraphQL spec
        // allows auto-coercion of an object to a single element list.

        let td = self
            .schema
            .get_type_definition(underlying.as_str())
            .unwrap();
        let input_object_type = match &td.kind {
            TypeKind::InputObject(input_object_type) => Ok(input_object_type),
            _ => Err(ValidationError::InvalidArgumentType {
                argument_name: argument_definition.name.node.to_string(),
                expected_type: ty.to_string(),
                actual_type: td.name.to_string(),
                pos,
            }),
        }?;

        let field_arguments: Vec<_> = entires
            .iter()
            .map(|(name, v)| {
                (
                    Positioned::new(name.clone(), pos),
                    Positioned::new(v.clone(), pos),
                )
            })
            .collect::<Vec<_>>();

        let validated_arguments = self.validate_arguments(
            &input_object_type
                .fields
                .iter()
                .map(|d| &d.node)
                .collect::<Vec<_>>(),
            &field_arguments,
        )?;

        let map = validated_arguments.into_iter().collect::<HashMap<_, _>>();

        Ok(Val::Object(map))
    }

    fn validate_list_argument(
        &self,
        argument_definition: &InputValueDefinition,
        elems: &[Value],
        pos: Pos,
    ) -> Result<Val, ValidationError> {
        let ty = &argument_definition.ty.node;
        let underlying = underlying_type(ty);

        // If the expected type is json, treat it as an opaque object
        if underlying.as_str() == "Json" {
            let unresolved_value: UnresolvedVal = Value::List(elems.to_vec()).into();
            return unresolved_value.resolve(&|name: &str| {
                self.variables
                    .get(&Name::new(name))
                    .cloned()
                    .ok_or_else(|| {
                        ValidationError::VariableNotFound(name.to_string(), Pos::default())
                    })
            });
        }

        match &ty.base {
            BaseType::Named(name) => Err(ValidationError::InvalidArgumentType {
                argument_name: argument_definition.name.node.to_string(),
                expected_type: underlying.to_string(),
                actual_type: format!("[{name}]"),
                pos,
            }),
            BaseType::List(elem_type) => {
                // Peel off the list type to get the element type

                let elem_argument_definition = InputValueDefinition {
                    ty: Positioned::new(elem_type.as_ref().clone(), pos),
                    ..argument_definition.clone()
                };

                let validated_elems = elems
                    .iter()
                    .flat_map(|elem| {
                        self.validate_argument(
                            &elem_argument_definition,
                            Some(&Positioned::new(elem.clone(), pos)),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Val::List(validated_elems))
            }
        }
    }
}

fn validate_int_range(
    directives: Vec<Positioned<ConstDirective>>,
    value: i64,
    pos: Pos,
) -> Result<(), ValidationError> {
    let mut min: Option<i64> = None;
    let mut max: Option<i64> = None;
    if let Some(range) = directives.iter().find(|x| x.node.name.node == "range") {
        if let Some(x) = range.node.arguments.iter().find(|x| x.0.node == "min")
            && let ConstValue::Number(n) = &x.1.node
        {
            min = n.to_owned().as_i64();
        }
        if let Some(x) = range.node.arguments.iter().find(|x| x.0.node == "max")
            && let ConstValue::Number(n) = &x.1.node
        {
            max = n.to_owned().as_i64();
        }
    }

    validate_generic_range((min, max), value, pos)
}

fn validate_float_range(
    directives: Vec<Positioned<ConstDirective>>,
    value: f64,
    pos: Pos,
) -> Result<(), ValidationError> {
    let mut min: Option<f64> = None;
    let mut max: Option<f64> = None;
    if let Some(range) = directives.iter().find(|x| x.node.name.node == "range") {
        if let Some(x) = range.node.arguments.iter().find(|x| x.0.node == "min")
            && let ConstValue::Number(n) = &x.1.node
        {
            min = n.to_owned().as_f64();
        }
        if let Some(x) = range.node.arguments.iter().find(|x| x.0.node == "max")
            && let ConstValue::Number(n) = &x.1.node
        {
            max = n.to_owned().as_f64();
        }
    }

    validate_generic_range((min, max), value, pos)
}

fn validate_generic_range<T>(
    range: (Option<T>, Option<T>),
    value: T,
    pos: Pos,
) -> Result<(), ValidationError>
where
    T: std::cmp::PartialOrd + std::fmt::Display,
{
    let (min, max) = range;
    if let Some(r) = min
        && r > value
    {
        return Err(ValidationError::ValueOutOfRange {
            value_name: "range".into(),
            range_detail: format!("min = {}", r),
            value_detail: format!("provided value = {}", value),
            pos,
        });
    }
    if let Some(r) = max
        && value > r
    {
        return Err(ValidationError::ValueOutOfRange {
            value_name: "range".into(),
            range_detail: format!("max = {}", r),
            value_detail: format!("provided value = {}", value),
            pos,
        });
    }

    Ok(())
}
