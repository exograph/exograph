use std::collections::HashMap;

use async_graphql_parser::{
    types::{
        Field, FieldDefinition, FragmentDefinition, FragmentSpread, Selection, SelectionSet, Type,
        TypeDefinition,
    },
    Positioned,
};
use async_graphql_value::{indexmap::IndexMap, ConstValue, Name};

use crate::validation::field::ValidatedField;
use payas_core_model::type_normalization::TypeDefinitionIntrospection;

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
        selection_set
            .node
            .items
            .iter()
            .map(|selection| self.validate_selection(selection))
            .collect::<Result<Vec<_>, _>>()
            .map(|f| f.into_iter().flatten().collect())
    }

    fn validate_selection(
        &self,
        selection: &Positioned<Selection>,
    ) -> Result<Vec<ValidatedField>, ValidationError> {
        match &selection.node {
            Selection::Field(field) => self.validate_field(field).map(|field| vec![field]),
            Selection::FragmentSpread(fragment_spread) => self
                .fragment_definition(fragment_spread)
                .and_then(|fragment_definition| self.validate(&fragment_definition.selection_set)),
            Selection::InlineFragment(inline_fragment) => Err(
                ValidationError::InlineFragmentNotSupported(inline_fragment.pos),
            ),
        }
    }

    fn validate_field(&self, field: &Positioned<Field>) -> Result<ValidatedField, ValidationError> {
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
                Ok(ValidatedField {
                    alias: field.node.alias.as_ref().map(|alias| alias.node.clone()),
                    name: field.node.name.node.clone(),
                    arguments: IndexMap::new(),
                    subfields: vec![],
                })
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

            Ok(ValidatedField {
                alias: field.node.alias.as_ref().map(|alias| alias.node.clone()),
                name: field.node.name.node.clone(),
                arguments,
                subfields,
            })
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
