use std::collections::HashMap;

use async_graphql_parser::{types::Field, Pos, Positioned};
use async_graphql_value::{indexmap::IndexMap, ConstValue, Name, Number, Value};
use bytes::Bytes;
use payas_model::model::system::ModelSystem;

use crate::graphql::validation::validation_error::ValidationError;

use super::{
    definition::{
        GqlFieldDefinition, GqlFieldTypeDefinition, GqlFieldTypeDefinitionNode, GqlTypeDefinition,
    },
    find_arg_type, TypeModifier,
};

pub struct ArgumentValidator<'a> {
    model: &'a ModelSystem,
    variables: &'a HashMap<Name, ConstValue>,
    field: &'a Positioned<Field>,
}

impl<'a> ArgumentValidator<'a> {
    #[must_use]
    pub fn new(
        model: &'a ModelSystem,
        variables: &'a HashMap<Name, ConstValue>,
        field: &'a Positioned<Field>,
    ) -> Self {
        Self {
            model,
            variables,
            field,
        }
    }

    /// Validations performed:
    /// - Ensure that all required arguments are provided
    /// - Ensure that there are no stray arguments (arguments that are not defined in the field)
    /// - TODO: Ensure that the argument type is compatible with the argument definition
    ///         (currently, we do a partial check: object-shape, matched scalar, but no checks such
    ///          as a LocalTime argument is valid or the numbers fit the expected range).
    pub(super) fn validate(
        &self,
        field_argument_definition: &[&dyn GqlFieldDefinition],
    ) -> Result<IndexMap<String, ConstValue>, ValidationError> {
        self.validate_arguments(field_argument_definition, &self.field.node.arguments)
    }

    fn validate_arguments(
        &self,
        field_argument_definitions: &[&dyn GqlFieldDefinition],
        field_arguments: &[(Positioned<Name>, Positioned<Value>)],
    ) -> Result<IndexMap<String, ConstValue>, ValidationError> {
        let field_name = self.field.node.name.node.as_str();

        // Stray arguments tracking: 1. Maintain a hashmap of all the arguments supplied in the query
        let mut field_arguments: HashMap<_, _> = field_arguments
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
                    Some((name.node.to_string(), value))
                }
            })
            .collect();

        let validated_arguments = field_argument_definitions
            .iter()
            .filter_map(|argument_definition| {
                let argument_name = &argument_definition.name();
                // Stray arguments tracking: 2. Remove the argument being processed
                let argument_value = field_arguments.remove(argument_name.to_owned());

                self.validate_argument(*argument_definition, argument_value)
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
        argument_definition: &dyn GqlFieldDefinition,
        argument_value: Option<&Positioned<Value>>,
    ) -> Option<Result<ConstValue, ValidationError>> {
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
                Value::Enum(e) => Some(Ok(ConstValue::Enum(e.clone()))),
                Value::List(elems) => {
                    Some(self.validate_list_argument(argument_definition, elems, value.pos))
                }
                Value::Object(object) => {
                    Some(self.validate_object_argument(argument_definition, object, value.pos))
                }
            },
            None => match argument_definition.field_type(self.model).modifier() {
                // If the expected type is optional or list, we can skip the argument
                // TODO: For the list case, we should be able to structure PredicateType better.
                //       Currently, we lack the ability to specify Option<List<T>> due to
                //       GqlTypeModifier (which allows only one modifier on a base type), so we
                //       specify List<T>. This means that we can't distinguish between a list with a
                //       single null element and an empty list.
                TypeModifier::Optional | TypeModifier::List => None,
                _ => Some(Err(ValidationError::RequiredArgumentNotFound(
                    argument_definition.name().to_owned(),
                    self.field.pos,
                ))),
            },
        }
    }

    fn validate_null_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        let ty = &argument_definition.field_type(self.model);

        match ty.modifier() {
            TypeModifier::Optional => Ok(ConstValue::Null),
            _ => Err(ValidationError::RequiredArgumentNotFound(
                argument_definition.name().to_owned(),
                pos,
            )),
        }
    }

    fn validate_number_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        number: &Number,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        // TODO: Use the types from PrimitiveType (but that is currently in the payas-parser crate, which we don't want to depend on)
        self.validate_scalar_argument(
            "Number",
            &["Int", "Float"],
            || ConstValue::Number(number.clone()),
            argument_definition,
            pos,
        )
    }

    fn validate_boolean_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        boolean: &bool,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        // TODO: Use the types from PrimitiveType (but that is currently in the payas-parser crate, which we don't want to depend on)
        self.validate_scalar_argument(
            "Boolean",
            &["Boolean"],
            || ConstValue::Boolean(*boolean),
            argument_definition,
            pos,
        )
    }

    fn validate_string_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        string: &str,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        // TODO: Use the types from PrimitiveType (but that is currently in the payas-parser crate, which we don't want to depend on)
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
                "Blob",
                "Json",
            ],
            || ConstValue::String(string.to_string()),
            argument_definition,
            pos,
        )
    }

    fn validate_binary_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        bytes: &Bytes,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        self.validate_scalar_argument(
            "Binary",
            &["Binary"],
            || ConstValue::Binary(bytes.clone()),
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
        to_const_value: impl FnOnce() -> ConstValue,
        argument_definition: &dyn GqlFieldDefinition,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        let ty = &argument_definition.field_type(self.model);

        let underlying = ty.name(self.model);

        if acceptable_destination_types.contains(&underlying) {
            Ok(to_const_value())
        } else {
            Err(ValidationError::InvalidArgumentType {
                argument_name: argument_definition.name().to_owned(),
                expected_type: underlying.to_string(),
                actual_type: argument_typename.to_string(),
                pos,
            })
        }
    }

    /// Recursively validate an object argument
    fn validate_object_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        entires: &IndexMap<Name, Value>,
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        let ty = &argument_definition.field_type(self.model);
        let field_underlying_type_name = ty.name(self.model);

        if field_underlying_type_name == "Json" {
            let const_value = Value::Object(entires.clone()).into_const_with(|name| {
                self.variables.get(&name).cloned().ok_or_else(|| {
                    ValidationError::VariableNotFound(name.to_string(), Pos::default())
                })
            });
            return const_value;
        }

        // We don't validate if the expected type is an object (and not a list), since the GraphQL spec
        // allows auto-coercion of an object to a single element list.

        let field_underlying_type = find_arg_type(self.model, field_underlying_type_name);

        let field_underlying_type: &dyn GqlTypeDefinition = match field_underlying_type {
            Some(field_underlying_type) => field_underlying_type,
            None => {
                return Err(ValidationError::InvalidArgumentType {
                    argument_name: argument_definition.name().to_owned(),
                    expected_type: field_underlying_type_name.to_string(),
                    actual_type: field_underlying_type_name.to_string(),
                    pos,
                });
            }
        };

        let field_arguments: Vec<_> = entires
            .iter()
            .map(|(name, v)| {
                (
                    Positioned::new(name.clone(), pos),
                    Positioned::new(v.clone(), pos),
                )
            })
            .collect::<Vec<_>>();

        let validated_arguments =
            self.validate_arguments(&field_underlying_type.fields(self.model), &field_arguments)?;

        let index_map = validated_arguments
            .into_iter()
            .map(|(k, v)| (Name::new(k), v))
            .collect::<IndexMap<_, _>>();

        Ok(ConstValue::Object(index_map))
    }

    fn validate_list_argument(
        &self,
        argument_definition: &dyn GqlFieldDefinition,
        elems: &[Value],
        pos: Pos,
    ) -> Result<ConstValue, ValidationError> {
        let field_type = &argument_definition.field_type(self.model);
        let underlying_field_type_name = field_type.name(self.model);

        // If the expected type is json, treat it as an opaque object
        if underlying_field_type_name == "Json" {
            let const_value = Value::List(elems.to_vec()).into_const_with(|name| {
                self.variables.get(&name).cloned().ok_or_else(|| {
                    ValidationError::VariableNotFound(name.to_string(), Pos::default())
                })
            });
            return const_value;
        }

        match field_type.modifier() {
            TypeModifier::NonNull => Err(ValidationError::InvalidArgumentType {
                argument_name: argument_definition.name().to_string(),
                expected_type: argument_definition
                    .field_type(self.model)
                    .name(self.model)
                    .to_string(),
                actual_type: format!("[{}]", field_type.name(self.model)),
                pos,
            }),
            TypeModifier::Optional => self.validate_list_argument(
                &get_inner_field_definition(argument_definition, self.model),
                elems,
                pos,
            ),
            TypeModifier::List => {
                let validated_elems = elems
                    .iter()
                    .flat_map(|elem| {
                        self.validate_argument(
                            &get_inner_field_definition(argument_definition, self.model),
                            Some(&Positioned::new(elem.clone(), pos)),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(ConstValue::List(validated_elems))
            }
        }
    }
}

fn get_inner_field_definition<'a>(
    field_definition: &'a dyn GqlFieldDefinition,
    model: &'a ModelSystem,
) -> InnerGqlFieldDefinition<'a> {
    let typ = field_definition.field_type(model);

    let inner_field_type_definition = match typ.inner(model) {
        GqlFieldTypeDefinitionNode::NonLeaf(inner, ..) => inner,
        GqlFieldTypeDefinitionNode::Leaf(_) => field_definition.field_type(model),
    };

    InnerGqlFieldDefinition {
        name: field_definition.name(),
        field_type_definition: inner_field_type_definition,
    }
}

#[derive(Debug)]
struct InnerGqlFieldDefinition<'a> {
    name: &'a str,
    field_type_definition: &'a dyn GqlFieldTypeDefinition,
}

impl<'b> GqlFieldDefinition for InnerGqlFieldDefinition<'b> {
    fn name(&self) -> &str {
        self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        self.field_type_definition
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![]
    }
}
