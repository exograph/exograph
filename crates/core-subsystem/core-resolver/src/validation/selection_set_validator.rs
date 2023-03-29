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
    is_introspection: Option<bool>,
}

impl<'a> SelectionSetValidator<'a> {
    #[must_use]
    pub fn new(
        schema: &'a Schema,
        container_type: &'a TypeDefinition,
        variables: &'a HashMap<Name, ConstValue>,
        fragment_definitions: &'a HashMap<Name, Positioned<FragmentDefinition>>,
        is_introspection: Option<bool>,
    ) -> Self {
        Self {
            schema,
            container_type,
            variables,
            fragment_definitions,
            is_introspection,
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
        selection_depth: usize,
        selection_depth_check: &impl Fn(usize, Option<bool>, Pos) -> Result<bool, ValidationError>,
    ) -> Result<Vec<ValidatedField>, ValidationError> {
        self.validate_selection_set(
            selection_set,
            HashSet::new(),
            selection_depth,
            selection_depth_check,
        )
        .map(|fields| fields.into_iter().map(|(_, field)| field).collect())
    }

    fn validate_selection_set(
        &self,
        selection_set: &Positioned<SelectionSet>,
        fragment_trail: HashSet<String>,
        selection_depth: usize,
        selection_depth_check: &impl Fn(usize, Option<bool>, Pos) -> Result<bool, ValidationError>,
    ) -> Result<Vec<(Pos, ValidatedField)>, ValidationError> {
        let fields = selection_set.node.items.iter().map(|selection| {
            self.validate_selection(
                selection,
                fragment_trail.clone(),
                selection_depth,
                selection_depth_check,
            )
        });

        let fields: Vec<(Pos, ValidatedField)> = fields
            .collect::<Result<Vec<_>, _>>()
            .map(|f| f.into_iter().flatten().collect())?;

        // Merge any duplicate fields (see https://spec.graphql.org/October2021/#sec-Field-Selection-Merging)

        // First gather all fields by name. This is a map of field name to a list of fields with that name.
        let mut fields_map: IndexMap<String, Vec<(Pos, ValidatedField)>> = IndexMap::new();
        for field in fields {
            let name = field.1.output_name();

            match fields_map.get_mut(&name) {
                Some(fields) => {
                    fields.push(field);
                }
                None => {
                    fields_map.insert(name, vec![field]);
                }
            }
        }

        // Second, merge the fields with the same name. If there is only one field, it is trivially merged (return that single entry)
        let fields = fields_map.into_values().map(|mut fields| {
            if fields.len() == 1 {
                Ok(fields.remove(0))
            } else {
                Self::merge_fields(fields)
            }
        });

        let mut valid_fields = vec![];
        let mut invalid_fields = vec![];

        for field in fields {
            match field {
                Ok(field) => valid_fields.push(field),
                Err(err) => invalid_fields.push(err),
            }
        }

        if invalid_fields.is_empty() {
            Ok(valid_fields)
        } else {
            Err(invalid_fields.remove(0))
        }
    }

    fn merge_fields(
        mut fields: Vec<(Pos, ValidatedField)>,
    ) -> Result<(Pos, ValidatedField), ValidationError> {
        let mut acc = Ok(fields.remove(0));

        for (next_field_pos, next_field) in fields {
            match acc {
                Ok((field_pos, field)) => {
                    acc = merge_field(field, next_field, vec![field_pos, next_field_pos])
                        .map(|f| (field_pos, f));
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        acc
    }

    fn validate_selection(
        &self,
        selection: &Positioned<Selection>,
        fragment_trail: HashSet<String>,
        selection_depth: usize,
        selection_depth_check: &impl Fn(usize, Option<bool>, Pos) -> Result<bool, ValidationError>,
    ) -> Result<Vec<(Pos, ValidatedField)>, ValidationError> {
        match &selection.node {
            Selection::Field(field) => self
                .validate_field(field, selection_depth, selection_depth_check)
                .map(|field| vec![field]),
            Selection::FragmentSpread(fragment_spread) => {
                if fragment_trail.contains(&fragment_spread.node.fragment_name.node.to_string()) {
                    return Err(ValidationError::FragmentCycle(
                        fragment_spread.node.fragment_name.node.to_string(),
                        fragment_spread.pos,
                    ));
                }
                let fragment_trail = {
                    let mut fragment_trail = fragment_trail;
                    fragment_trail.insert(fragment_spread.node.fragment_name.node.to_string());
                    fragment_trail
                };
                self.fragment_definition(fragment_spread)
                    .and_then(|fragment_definition| {
                        self.validate_selection_set(
                            &fragment_definition.selection_set,
                            fragment_trail,
                            selection_depth,
                            selection_depth_check,
                        )
                    })
            }
            Selection::InlineFragment(inline_fragment) => Err(
                ValidationError::InlineFragmentNotSupported(inline_fragment.pos),
            ),
        }
    }

    fn validate_field(
        &self,
        field: &Positioned<Field>,
        selection_depth: usize,
        selection_depth_check: &impl Fn(usize, Option<bool>, Pos) -> Result<bool, ValidationError>,
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
            let (field_definition, is_introspection) =
                if self.container_type.name.node.as_str() == QUERY_ROOT_TYPENAME {
                    // We have to treat the query root type specially, since its __schema and __type fields are not
                    // "ordinary" fields, but are instead special-cased in the introspection query (much the same way
                    // as the __typename field).
                    if field.node.name.node.as_str() == "__schema" {
                        (&self.schema.schema_field_definition, true)
                    } else if field.node.name.node.as_str() == "__type" {
                        (&self.schema.type_field_definition, true)
                    } else {
                        (self.get_field_definition(field)?, false)
                    }
                } else {
                    (self.get_field_definition(field)?, false)
                };

            let field_type_definition = self.get_type_definition(&field_definition.ty, field)?;
            let is_introspection: Option<bool> = self.is_introspection.or(Some(is_introspection));

            let subfield_validator = SelectionSetValidator::new(
                self.schema,
                field_type_definition,
                self.variables,
                self.fragment_definitions,
                is_introspection,
            );

            let new_selection_depth = selection_depth + 1;
            selection_depth_check(new_selection_depth, is_introspection, field.pos)?;

            let subfields = subfield_validator.validate(
                &field.node.selection_set,
                new_selection_depth,
                selection_depth_check,
            )?;

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

pub fn merge_field(
    first: ValidatedField,
    second: ValidatedField,
    positions: Vec<Pos>,
) -> Result<ValidatedField, ValidationError> {
    if first.output_name() != second.output_name()
        || first.name != second.name
        || first.arguments != second.arguments
    {
        return Err(ValidationError::MergeFields(first.output_name(), positions));
    }

    let field_output_name = first.output_name();

    match merge_subfields(first.subfields, second.subfields) {
        Ok(subfields) => Ok(ValidatedField { subfields, ..first }),
        Err(_) => Err(ValidationError::MergeFields(field_output_name, positions)),
    }
}

fn merge_subfields(
    fields: Vec<ValidatedField>,
    mut other_fields: Vec<ValidatedField>,
) -> Result<Vec<ValidatedField>, Vec<ValidationError>> {
    let mut merged_fields = vec![];

    // Merged others into fields
    fields.into_iter().for_each(|field| {
        let matching_index = other_fields
            .iter()
            .position(|other_field| other_field.output_name() == field.output_name());

        let merged_field = match matching_index {
            Some(index) => {
                let other_field = other_fields.remove(index);
                merge_field(field, other_field, vec![])
            }
            None => Ok(field),
        };

        merged_fields.push(merged_field)
    });

    // Since we removed all matching fields from `other_fields`, the remaining fields can be added as is
    merged_fields.extend(other_fields.into_iter().map(Ok));

    let mut valid_fields = vec![];
    let mut invalid_fields = vec![];
    for field in merged_fields {
        match field {
            Ok(field) => valid_fields.push(field),
            Err(error) => invalid_fields.push(error),
        }
    }

    if invalid_fields.is_empty() {
        Ok(valid_fields)
    } else {
        Err(invalid_fields)
    }
}
