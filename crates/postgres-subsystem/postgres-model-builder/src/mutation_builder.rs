//! Build mutation input types (`<Type>CreationInput`, `<Type>UpdateInput`, `<Type>ReferenceInput`) and
//! mutations (`create<Type>`, `update<Type>`, and `delete<Type>` as well as their plural versions)

use core_plugin_interface::core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{BaseOperationReturnType, FieldType, Named, OperationReturnType},
};

use postgres_model::{
    mutation::{PostgresMutation, PostgresMutationParameters},
    relation::PostgresRelation,
    types::{
        base_type, EntityType, MutationType, PostgresField, PostgresFieldType, PostgresType,
        TypeIndex,
    },
};

use crate::{
    resolved_builder::{ResolvedField, ResolvedFieldTypeHelper},
    shallow::Shallow,
    utils::to_mutation_type,
};

use super::{
    builder::Builder,
    create_mutation_builder::CreateMutationBuilder,
    delete_mutation_builder::DeleteMutationBuilder,
    naming::ToPostgresTypeNames,
    reference_input_type_builder::ReferenceInputTypeBuilder,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
    update_mutation_builder::UpdateMutationBuilder,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataParamRole {
    Create,
    Update,
}

// TODO: Introduce this module as a struct (and have it hold the sub-builders)
// TODO: Abstract the concept of composite builders

/// Build shallow mutation input types
pub fn build_shallow(
    resolved_types: &MappedArena<ResolvedType>,
    building: &mut SystemContextBuilding,
) {
    ReferenceInputTypeBuilder {}.build_shallow(resolved_types, building);

    CreateMutationBuilder {}.build_shallow(resolved_types, building);
    UpdateMutationBuilder {}.build_shallow(resolved_types, building);
    DeleteMutationBuilder {}.build_shallow(resolved_types, building);
}

/// Expand the mutation input types as well as build the mutation
pub fn build_expanded(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_expanded(resolved_env, building); // Used by many...

    CreateMutationBuilder {}.build_expanded(resolved_env, building);
    UpdateMutationBuilder {}.build_expanded(resolved_env, building);
    DeleteMutationBuilder {}.build_expanded(resolved_env, building);
}

pub trait MutationBuilder {
    fn single_mutation_name(entity_type: &EntityType) -> String;
    fn single_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters;
    fn single_mutation_modified_type(
        base_type: BaseOperationReturnType<EntityType>,
    ) -> OperationReturnType<EntityType>;

    fn multi_mutation_name(entity_type: &EntityType) -> String;
    fn multi_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters;

    fn build_mutations(
        &self,
        entity_type_id: SerializableSlabIndex<EntityType>,
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> Vec<PostgresMutation> {
        let single_mutation = PostgresMutation {
            name: Self::single_mutation_name(entity_type),
            parameters: Self::single_mutation_parameters(entity_type, building),
            return_type: Self::single_mutation_modified_type(BaseOperationReturnType {
                associated_type_id: entity_type_id,
                type_name: entity_type.name.clone(),
            }),
        };

        let multi_mutation = PostgresMutation {
            name: Self::multi_mutation_name(entity_type),
            parameters: Self::multi_mutation_parameters(entity_type, building),
            return_type: OperationReturnType::List(Box::new(OperationReturnType::Plain(
                BaseOperationReturnType {
                    associated_type_id: entity_type_id,
                    type_name: entity_type.name.clone(),
                },
            ))),
        };

        vec![single_mutation, multi_mutation]
    }
}

pub trait DataParamBuilder<D> {
    fn data_param_type_name(resolved_composite_type: &ResolvedCompositeType) -> String {
        Self::base_data_type_name(&resolved_composite_type.name)
    }

    fn base_data_type_name(entity_type_name: &str) -> String;

    fn data_param(entity_type: &EntityType, building: &SystemContextBuilding, array: bool) -> D;

    fn data_type_name(entity_type_name: &str, container_type: Option<&str>) -> String {
        let base_name = Self::base_data_type_name(entity_type_name);
        super::mutation_builder::data_type_name(&base_name, container_type)
    }

    fn data_param_role() -> DataParamRole;

    fn compute_data_fields(
        &self,
        postgres_fields: &[PostgresField<EntityType>],
        top_level_type: Option<&EntityType>,
        container_type: Option<&str>,
        building: &SystemContextBuilding,
    ) -> Vec<PostgresField<MutationType>> {
        postgres_fields
            .iter()
            .flat_map(|field| {
                self.compute_data_field(field, top_level_type, container_type, building)
            })
            .collect()
    }

    // TODO: Revisit this after nested update mutation works
    fn mark_fields_optional() -> bool;

    fn data_param_field_type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        resolved_types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        resolved_composite_type
            .fields
            .iter()
            .flat_map(|field| {
                // Create a nested input data type only if it refers to a many side
                // So for Venue <-> [Concert] case, create only ConcertCreationInputFromVenue

                // we can treat Optional fields as their inner type for the purposes of
                // computing their type names
                let typ = match &field.typ {
                    FieldType::Optional(inner_type) => inner_type.as_ref(),
                    _ => &field.typ,
                };

                // If the type is a list or a reference, we need to create a nested input type (one-to-many or one-to-zero-or-one)
                if let Some(ResolvedType::Composite(ResolvedCompositeType { name, .. })) =
                    typ.deref_subsystem_type(resolved_types)
                {
                    if let FieldType::List(_) = field.typ {
                        // If it is a list, we need to create a nested input type (one-to-many)
                        Self::data_param_field_one_to_many_type_names(name, resolved_composite_type)
                    } else if let FieldType::Optional(_) = field.typ {
                        // Let's determine if it is one-to-zero_or_one (where we need to create a nested input type)
                        // Or many-to-one_optional (Think Concert with an optional Venue, and Venue with multiple (possibly optional) concerts)
                        match get_matching_field(field, resolved_types) {
                            Some(matching_field) => {
                                let inner_type = matching_field.typ.inner();
                                if let Some(FieldType::List(_)) = inner_type {
                                    vec![]
                                } else {
                                    Self::data_param_field_one_to_many_type_names(
                                        name,
                                        resolved_composite_type,
                                    )
                                }
                            }
                            None => {
                                vec![]
                            }
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            })
            .collect()
    }

    fn data_param_field_one_to_many_type_names(
        field_type_name: &str,
        resolved_composite_type: &ResolvedCompositeType,
    ) -> Vec<String> {
        vec![Self::data_type_name(
            field_type_name,
            Some(&resolved_composite_type.name),
        )]
    }

    fn compute_data_field(
        &self,
        field: &PostgresField<EntityType>,
        top_level_type: Option<&EntityType>,
        container_type: Option<&str>,
        building: &SystemContextBuilding,
    ) -> Option<PostgresField<MutationType>> {
        let optional = Self::mark_fields_optional() || field.has_default_value;

        match &field.relation {
            PostgresRelation::Pk { .. } => {
                if Self::data_param_role() == DataParamRole::Update {
                    // A typical way clients use update mutation is to get the data along with the id,
                    // modify the data and send it back to the server. So we should accept the id
                    // as an optional field in the update mutation.
                    // See also https://github.com/payalabs/payas/issues/601
                    Some(PostgresField {
                        name: field.name.clone(),
                        typ: to_mutation_type(&field.typ).optional(),
                        relation: field.relation.clone(),
                        has_default_value: field.has_default_value,
                    })
                } else {
                    // TODO: Make this decision based on autoIncrement/uuid etc of the id
                    None
                }
            }
            PostgresRelation::Scalar { .. } => Some(PostgresField {
                name: field.name.clone(),
                typ: if optional {
                    to_mutation_type(&field.typ).optional()
                } else {
                    to_mutation_type(&field.typ)
                },
                relation: field.relation.clone(),
                has_default_value: field.has_default_value,
            }),
            PostgresRelation::OneToMany { .. } => {
                self.compute_one_to_many_data_field(field, container_type, building)
            }
            PostgresRelation::ManyToOne { .. } => {
                let field_type_name = field.typ.name().reference_type();
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = FieldType::Plain(PostgresFieldType {
                    type_name: field_type_name,
                    type_id: TypeIndex::Composite(field_type_id),
                });
                let field_type = match field.typ {
                    FieldType::Plain(_) => {
                        if optional {
                            field_plain_type.optional()
                        } else {
                            field_plain_type
                        }
                    }
                    FieldType::Optional(_) => FieldType::Optional(Box::new(field_plain_type)),
                    FieldType::List(_) => FieldType::List(Box::new(field_plain_type)),
                };

                match &top_level_type {
                    Some(value) if value.name == field.typ.name() => None,
                    _ => Some(PostgresField {
                        name: field.name.clone(),
                        typ: field_type,
                        relation: field.relation.clone(),
                        has_default_value: field.has_default_value,
                    }),
                }
            }
        }
    }

    fn compute_one_to_many_data_field(
        &self,
        field: &PostgresField<EntityType>,
        container_type: Option<&str>,
        building: &SystemContextBuilding,
    ) -> Option<PostgresField<MutationType>> {
        let optional = matches!(field.typ, FieldType::Optional(_)) || Self::mark_fields_optional();

        let field_type_name = Self::data_type_name(field.typ.name(), container_type);

        building
            .mutation_types
            .get_id(&field_type_name)
            .and_then(|field_type_id| {
                let field_plain_type = FieldType::Plain(PostgresFieldType {
                    type_name: field_type_name,
                    type_id: TypeIndex::Composite(field_type_id),
                });
                let field_type = FieldType::List(Box::new(field_plain_type));

                match &container_type {
                    Some(value) if value == &field.typ.name() => None,
                    _ => Some(PostgresField {
                        name: field.name.clone(),
                        typ: if optional {
                            field_type.optional()
                        } else {
                            field_type
                        },
                        relation: field.relation.clone(),
                        has_default_value: field.has_default_value,
                    }),
                }
            })
    }

    fn expanded_data_type(
        &self,
        entity_type: &EntityType,
        resolved_env: &ResolvedTypeEnv,
        building: &SystemContextBuilding,
        top_level_type: Option<&EntityType>,
        container_type: Option<&EntityType>,
    ) -> Vec<(SerializableSlabIndex<MutationType>, MutationType)> {
        let mut field_types: Vec<_> = entity_type
            .fields
            .iter()
            .flat_map(|field| {
                let field_type = base_type(
                    &field.typ,
                    &building.primitive_types.values,
                    &building.entity_types.values,
                );
                if let (PostgresType::Composite(field_type), PostgresRelation::OneToMany { .. }) =
                    (&field_type, &field.relation)
                {
                    self.expand_one_to_many(
                        entity_type,
                        field,
                        field_type,
                        resolved_env,
                        building,
                        top_level_type,
                        Some(entity_type),
                    )
                } else {
                    vec![]
                }
            })
            .collect();

        let existing_type_name = Self::data_type_name(
            entity_type.name.as_str(),
            container_type.map(|value| value.name.as_str()),
        );
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        let input_type_fields = self.compute_data_fields(
            &entity_type.fields,
            top_level_type,
            Some(entity_type.name.as_str()),
            building,
        );
        field_types.push((
            existing_type_id,
            MutationType {
                name: existing_type_name,
                fields: input_type_fields,
                entity_type: building.entity_types.get_id(&entity_type.name).unwrap(),
            },
        ));

        field_types
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_one_to_many(
        &self,
        entity_type: &EntityType,
        _field: &PostgresField<EntityType>,
        field_type: &EntityType,
        resolved_env: &ResolvedTypeEnv,
        building: &SystemContextBuilding,
        top_level_type: Option<&EntityType>,
        _container_type: Option<&EntityType>,
    ) -> Vec<(SerializableSlabIndex<MutationType>, MutationType)> {
        let new_container_type = Some(entity_type);

        let existing_type_name = Self::data_type_name(
            &field_type.name,
            new_container_type.map(|value| value.name.as_str()),
        );

        let existing = building
            .mutation_types
            .get_by_key(&existing_type_name)
            .unwrap_or_else(|| panic!("Could not find type {existing_type_name} to expand"));

        if existing.entity_type == SerializableSlabIndex::shallow() {
            // If not already expanded
            self.expanded_data_type(
                field_type,
                resolved_env,
                building,
                top_level_type,
                new_container_type,
            )
        } else {
            vec![]
        }
    }
}

pub fn create_data_type_name(entity_type_name: &str, container_type: Option<&str>) -> String {
    let base_name = entity_type_name.creation_type();
    data_type_name(&base_name, container_type)
}

pub fn update_data_type_name(entity_type_name: &str, container_type: Option<&str>) -> String {
    let base_name = entity_type_name.update_type();
    data_type_name(&base_name, container_type)
}

fn data_type_name(base_name: &str, container_type: Option<&str>) -> String {
    match container_type {
        Some(container_type) => {
            format!("{base_name}From{container_type}")
        }
        None => base_name.to_owned(),
    }
}

fn get_matching_field<'a>(
    field: &'a ResolvedField,
    types: &'a MappedArena<ResolvedType>,
) -> Option<&'a ResolvedField> {
    let field_typ = types.get_by_key(field.typ.name()).unwrap();

    if let ResolvedType::Composite(field_typ) = field_typ {
        let matching_fields: Vec<_> = field_typ
            .fields
            .iter()
            .filter(|f| field.column_name == f.column_name)
            .collect();

        match &matching_fields[..] {
            [matching_field] => Some(matching_field),
            _ => None,
        }
    } else {
        None
    }
}
