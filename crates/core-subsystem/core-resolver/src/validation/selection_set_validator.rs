use std::collections::{HashMap, HashSet};

use async_graphql_parser::{
    types::{
        Field, FieldDefinition, FragmentDefinition, FragmentSpread, Selection, SelectionSet, Type,
        TypeDefinition,
    },
    Pos, Positioned,
};
use async_graphql_value::{indexmap::IndexMap, ConstValue, Name};

use crate::validation::field::ValidatedField;
use core_model::type_normalization::TypeDefinitionIntrospection;

use crate::{
    introspection::definition::schema::{Schema, QUERY_ROOT_TYPENAME},
    validation::validation_error::ValidationError,
};

use super::{arguments_validator::ArgumentValidator, underlying_type};

/// Context for validating a selection set.
#[derive(Debug)]
pub struct SelectionSetValidator<'a> {
    schema: &'a Schema,
    /// The parent type of this field.
    container_type: &'a TypeDefinition,
    variables: &'a HashMap<Name, ConstValue>,
    fragment_definitions: &'a HashMap<Name, Positioned<FragmentDefinition>>,
}

impl<'a> SelectionSetValidator<'a> {
    #[must_use]
    pub fn new(
        schema: &'a Schema,
        container_type: &'a TypeDefinition,
        variables: &'a HashMap<Name, ConstValue>,
        fragment_definitions: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    ) -> Self {
        Self {
            schema,
            container_type,
            variables,
            fragment_definitions,
        }
    }

    /// Validate selection set.
    ///
    /// Validations performed:
    /// - Each field is defined in the `container_type`
    /// - Each fragment referred is defined
    /// - Arguments to each field are valid (see [validate_arguments] for more details)
    ///
    /// # Returns
    ///   A vector of validated fields (any fragment is resolved and inlined, thus normalizing the fields)
    pub(super) fn validate(
        &self,
        selection_set: &Positioned<SelectionSet>,
    ) -> Result<Vec<ValidatedField>, ValidationError> {
        self.validate_selection_set(selection_set)
            .map(|fields| fields.into_iter().map(|(_, field)| field).collect())
    }

    fn validate_selection_set(
        &self,
        selection_set: &Positioned<SelectionSet>,
    ) -> Result<Vec<(Pos, ValidatedField)>, ValidationError> {
        let fields = selection_set
            .node
            .items
            .iter()
            .map(|selection| self.validate_selection(selection));

        let fields: Vec<(Pos, ValidatedField)> = fields
            .collect::<Result<Vec<_>, _>>()
            .map(|f| f.into_iter().flatten().collect())?;

        // Validate that there are no duplicate fields output names (names considering aliases)
        let mut output_names = HashSet::new();
        let mut duplicated_names = HashSet::new();

        // First track any duplicated names
        for (_, field) in &fields {
            // HashSet::insert returns false if the value was already present
            if !output_names.insert(field.output_name()) {
                duplicated_names.insert(field.output_name());
            }
        }

        if duplicated_names.is_empty() {
            Ok(fields)
        } else {
            // For each duplicated name, gather its position (so we show the position for the every occurrence including the first one)
            let duplicated_positions = fields
                .iter()
                .flat_map(|(pos, field)| {
                    duplicated_names
                        .contains(&field.output_name())
                        .then_some(*pos)
                })
                .collect();
            let mut duplicated_names = duplicated_names.into_iter().collect::<Vec<_>>();
            duplicated_names.sort(); // Sort the names so the error message is deterministic
            Err(ValidationError::DuplicateFields(
                duplicated_names,
                duplicated_positions,
            ))
        }
    }

    fn validate_selection(
        &self,
        selection: &Positioned<Selection>,
    ) -> Result<Vec<(Pos, ValidatedField)>, ValidationError> {
        match &selection.node {
            Selection::Field(field) => self.validate_field(field).map(|field| vec![field]),
            Selection::FragmentSpread(fragment_spread) => self
                .fragment_definition(fragment_spread)
                .and_then(|fragment_definition| {
                    self.validate_selection_set(&fragment_definition.selection_set)
                }),
            Selection::InlineFragment(inline_fragment) => Err(
                ValidationError::InlineFragmentNotSupported(inline_fragment.pos),
            ),
        }
    }

    fn validate_field(
        &self,
        field: &Positioned<Field>,
    ) -> Result<(Pos, ValidatedField), ValidationError> {
        // Special treatment for the __typename field, since we are not supposed to expose it as
        // a normal field (for example, we should not declare that the "Concert" type has a __typename field")
        if field.node.name.node.as_str() == "__typename" {
            if !field.node.arguments.is_empty() {
                Err(ValidationError::StrayArguments(
                    field
                        .node
                        .arguments
                        .iter()
                        .map(|arg| arg.0.node.to_string())
                        .collect(),
                    field.node.name.to_string(),
                    field.pos,
                ))
            } else if !field.node.selection_set.node.items.is_empty() {
                Err(ValidationError::ScalarWithField(
                    field.node.name.to_string(),
                    field.pos,
                ))
            } else {
                Ok((
                    field.pos,
                    ValidatedField {
                        alias: field.node.alias.as_ref().map(|alias| alias.node.clone()),
                        name: field.node.name.node.clone(),
                        arguments: IndexMap::new(),
                        subfields: vec![],
                    },
                ))
            }
        } else {
            let field_definition = if self.container_type.name.node.as_str() == QUERY_ROOT_TYPENAME
            {
                // We have to treat the query root type specially, since its __schema and __type fields are not
                // "ordinary" fields, but are instead special-cased in the introspection query (much the same way
                // as the __typename field).
                if field.node.name.node.as_str() == "__schema" {
                    &self.schema.schema_field_definition
                } else if field.node.name.node.as_str() == "__type" {
                    &self.schema.type_field_definition
                } else {
                    self.get_field_definition(field)?
                }
            } else {
                self.get_field_definition(field)?
            };

            let field_type_definition = self.get_type_definition(&field_definition.ty, field)?;

            let subfield_validator = SelectionSetValidator::new(
                self.schema,
                field_type_definition,
                self.variables,
                self.fragment_definitions,
            );

            let subfields = subfield_validator.validate(&field.node.selection_set)?;

            let field_validator = ArgumentValidator::new(self.schema, self.variables, field);

            let arguments = field_validator.validate(
                &field_definition
                    .arguments
                    .iter()
                    .map(|d| &d.node)
                    .collect::<Vec<_>>(),
            )?;

            Ok((
                field.pos,
                ValidatedField {
                    alias: field.node.alias.as_ref().map(|alias| alias.node.clone()),
                    name: field.node.name.node.clone(),
                    arguments,
                    subfields,
                },
            ))
        }
    }

    fn fragment_definition(
        &self,
        fragment: &Positioned<FragmentSpread>,
    ) -> Result<&FragmentDefinition, ValidationError> {
        self.fragment_definitions
            .get(&fragment.node.fragment_name.node)
            .map(|v| &v.node)
            .ok_or_else(|| {
                ValidationError::FragmentDefinitionNotFound(
                    fragment.node.fragment_name.node.as_str().to_string(),
                    fragment.pos,
                )
            })
    }

    fn get_type_definition(
        &self,
        field_type: &Positioned<Type>,
        field: &Positioned<Field>,
    ) -> Result<&TypeDefinition, ValidationError> {
        let field_underlying_type_name = underlying_type(&field_type.node);
        let field_underlying_type = self
            .schema
            .get_type_definition(field_underlying_type_name.as_str());

        match field_underlying_type {
            None => Err(ValidationError::InvalidFieldType(
                field_underlying_type_name.as_str().to_string(),
                field.pos,
            )),
            Some(field_underlying_type) => Ok(field_underlying_type),
        }
    }

    fn get_field_definition(
        &'a self,
        field: &Positioned<Field>,
    ) -> Result<&FieldDefinition, ValidationError> {
        let field_definition = &self
            .container_type
            .fields()
            .and_then(|fields| fields.iter().find(|f| f.node.name == field.node.name))
            .map(|f| &f.node);

        match field_definition {
            None => Err(ValidationError::InvalidField(
                field.node.name.node.as_str().to_string(),
                self.container_type.name.node.to_string(),
                field.pos,
            )),
            Some(field_definition) => Ok(field_definition),
        }
    }
}
