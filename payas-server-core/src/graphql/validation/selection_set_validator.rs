use std::collections::HashMap;

use async_graphql_parser::{
    types::{Field, FragmentDefinition, FragmentSpread, Selection, SelectionSet},
    Pos, Positioned,
};
use async_graphql_value::{indexmap::IndexMap, ConstValue, Name};

use payas_model::model::system::ModelSystem;
use payas_resolver_core::validation::field::ValidatedField;

use crate::graphql::{
    introspection::definition::schema::Schema, validation::validation_error::ValidationError,
};

use super::{
    arguments_validator::ArgumentValidator,
    definition::{GqlFieldDefinition, GqlFieldTypeDefinition, GqlTypeDefinition},
};

/// Context for validating a selection set.
#[derive(Debug)]
pub struct SelectionSetValidator<'a> {
    model: &'a ModelSystem,
    schema: &'a Schema,
    /// The parent type of this field.
    container_type_definition: &'a dyn GqlTypeDefinition,
    variables: &'a HashMap<Name, ConstValue>,
    fragment_definitions: &'a HashMap<Name, Positioned<FragmentDefinition>>,
}

impl<'a> SelectionSetValidator<'a> {
    #[must_use]
    pub fn new(
        model: &'a ModelSystem,
        schema: &'a Schema,
        type_container: &'a dyn GqlTypeDefinition,
        variables: &'a HashMap<Name, ConstValue>,
        fragment_definitions: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    ) -> Self {
        Self {
            model,
            schema,
            container_type_definition: type_container,
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

    fn validate_field(
        &'a self,
        field: &Positioned<Field>,
    ) -> Result<ValidatedField, ValidationError> {
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
            let field_definition = self.get_field_definition(field)?;

            let field_type_definition =
                self.get_type_definition(field_definition.field_type(self.model), field.pos)?;

            let subfield_validator = Self::new(
                self.model,
                self.schema,
                field_type_definition,
                self.variables,
                self.fragment_definitions,
            );

            let subfields = subfield_validator.validate(&field.node.selection_set)?;

            let field_validator = ArgumentValidator::new(self.model, self.variables, field);

            let arguments = field_validator.validate(&field_definition.arguments(self.model))?;

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
        field_type: &dyn GqlFieldTypeDefinition,
        field_pos: Pos,
    ) -> Result<&dyn GqlTypeDefinition, ValidationError> {
        let field_underlying_type_name = &field_type.name(self.model);
        let field_underlying_type = self.find_type(field_underlying_type_name);

        match field_underlying_type {
            None => Err(ValidationError::InvalidFieldType(
                field_underlying_type_name.to_string(),
                field_pos,
            )),
            Some(field_underlying_type) => Ok(field_underlying_type),
        }
    }

    fn get_field_definition(
        &'a self,
        field: &Positioned<Field>,
    ) -> Result<&dyn GqlFieldDefinition, ValidationError> {
        let field_definition = &self
            .container_type_definition
            .fields(self.model)
            .into_iter()
            .find(|f| f.name() == field.node.name.node.as_str());

        let field_definition = match field_definition {
            Some(field_definition) => Some(*field_definition),
            // We have to treat the query root type specially, since its __schema and __type fields are not
            // "ordinary" fields, but are instead special-cased in the introspection query (much the same way
            // as the __typename field).
            None if field.node.name.node.as_str() == "__schema" => {
                Some(&self.schema.schema_field_definition as &dyn GqlFieldDefinition)
            }
            None if field.node.name.node.as_str() == "__type" => {
                Some(&self.schema.type_field_definition as &dyn GqlFieldDefinition)
            }
            None => None,
        };

        match field_definition {
            None => Err(ValidationError::InvalidField(
                field.node.name.node.as_str().to_owned(),
                self.container_type_definition.name().to_owned(),
                field.pos,
            )),
            Some(field_definition) => Ok(field_definition),
        }
    }

    fn find_type(&'a self, name: &str) -> Option<&'a dyn GqlTypeDefinition> {
        let core_type = self
            .model
            .types
            .iter()
            .find(|t| t.1.name.as_str() == name)
            .map(|t| t.1 as &dyn GqlTypeDefinition);

        match core_type {
            Some(t) => Some(t),
            None => self
                .schema
                .schema_type_definitions
                .iter()
                .find(|t| t.name() == name)
                .map(|t| t as &dyn GqlTypeDefinition),
        }
    }
}
