// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Build update mutation types `<Type>UpdateInput`, `update<Type>`, and `update<Type>s`

use core_model::{
    access::AccessPredicateExpression,
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{BaseOperationReturnType, FieldType, Named, OperationReturnType},
};
use core_model_builder::error::ModelBuildingError;
use postgres_graphql_model::{
    mutation::{DataParameter, DataParameterType, PostgresMutationParameters},
    types::MutationType,
};

use postgres_core_model::types::{EntityType, PostgresField, PostgresFieldType, TypeIndex};

use crate::{
    mutation_builder::DataParamRole,
    utils::{MutationTypeKind, to_mutation_type},
};

use postgres_core_builder::shallow::Shallow;

use super::{
    builder::Builder,
    mutation_builder::{DataParamBuilder, MutationBuilder},
    mutation_builder::{create_data_type_name, update_data_type_name},
    naming::{ToPostgresMutationNames, ToPostgresTypeNames},
    query_builder,
    system_builder::SystemContextBuilding,
};

use postgres_core_builder::resolved_type::{ResolvedCompositeType, ResolvedType};

pub struct UpdateMutationBuilder;

impl Builder for UpdateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        // TODO: This implementation is the same for CreateMutationBuilder. Fix it when we refactor non-mutations builders
        if !resolved_composite_type.access.update_allowed() {
            return vec![];
        }
        let mut field_types = self.data_param_field_type_names(resolved_composite_type, types);
        field_types.push(Self::data_param_type_name(resolved_composite_type));
        field_types
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(
        &self,
        building: &mut SystemContextBuilding,
    ) -> Result<(), ModelBuildingError> {
        let update_access_is_false = |entity_type: &EntityType| -> bool {
            matches!(
                building
                    .core_subsystem
                    .precheck_access_expressions
                    .lock()
                    .unwrap()[entity_type.access.update.precheck],
                AccessPredicateExpression::BooleanLiteral(false)
            ) || matches!(
                building
                    .core_subsystem
                    .database_access_expressions
                    .lock()
                    .unwrap()[entity_type.access.update.database],
                AccessPredicateExpression::BooleanLiteral(false)
            )
        };
        for (_, entity_type) in building.core_subsystem.entity_types.iter() {
            if !update_access_is_false(entity_type) {
                for (existing_id, expanded_type) in
                    self.expanded_data_type(entity_type, building, Some(entity_type), None, false)?
                {
                    building.mutation_types[existing_id] = expanded_type;
                }
            }
        }

        for (entity_type_id, entity_type) in building.core_subsystem.entity_types.iter() {
            if !update_access_is_false(entity_type) {
                for mutation in self.build_mutations(entity_type_id, entity_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
        }

        Ok(())
    }

    fn needs_mutation_type(&self, _composite_type: &ResolvedCompositeType) -> bool {
        true
    }
}

impl MutationBuilder for UpdateMutationBuilder {
    fn single_mutation_name(entity_type: &EntityType) -> String {
        entity_type.pk_update()
    }

    fn single_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters {
        PostgresMutationParameters::Update {
            data_param: Self::data_param(entity_type, building, false),
            predicate_params: query_builder::pk_predicate_params(
                entity_type,
                &building.core_subsystem.predicate_types,
                &building.core_subsystem.database,
            ),
        }
    }

    fn single_mutation_modified_type(
        base_type: BaseOperationReturnType<EntityType>,
    ) -> OperationReturnType<EntityType> {
        // We return null if the specified id doesn't exist
        OperationReturnType::Optional(Box::new(OperationReturnType::Plain(base_type)))
    }

    fn multi_mutation_name(entity_type: &EntityType) -> String {
        entity_type.collection_update()
    }

    fn multi_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters {
        PostgresMutationParameters::Update {
            data_param: Self::data_param(entity_type, building, true),
            predicate_params: vec![query_builder::collection_predicate_param(
                entity_type,
                &building.core_subsystem.predicate_types,
            )],
        }
    }

    fn single_mutation_doc_comments(entity_type: &EntityType) -> Option<String> {
        Some(format!(
            "Update the {} with the provided primary key with the provided data. Any fields not provided will remain unchanged.",
            entity_type.name
        ))
    }

    fn multi_mutation_doc_comments(entity_type: &EntityType) -> Option<String> {
        Some(format!(
            "Update multiple {}s matching the provided `where` filter with the provided data. Any fields not provided will remain unchanged.",
            entity_type.name
        ))
    }
}

impl DataParamBuilder<DataParameter> for UpdateMutationBuilder {
    fn mark_fields_optional() -> bool {
        true
    }

    fn use_list_for_nested_one_to_many() -> bool {
        false
    }

    fn base_data_type_name(entity_type_name: &str) -> String {
        entity_type_name.update_type()
    }

    fn data_param_role() -> DataParamRole {
        DataParamRole::Update
    }

    fn data_param(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
        _array: bool,
    ) -> DataParameter {
        let data_param_type_name = Self::base_data_type_name(&entity_type.name);
        let data_param_type_id = building
            .mutation_types
            .get_id(&data_param_type_name)
            .unwrap();

        DataParameter {
            name: "data".to_string(),
            typ: FieldType::Plain(DataParameterType {
                name: data_param_type_name,
                type_id: data_param_type_id,
                type_validation: None,
            }),
            type_validation: None,
            doc_comments: Some(format!("The data to update the {} with", entity_type.name)),
        }
    }

    fn data_param_field_one_to_many_type_names(
        field_type_name: &str,
        resolved_composite_type: &ResolvedCompositeType,
    ) -> Vec<String> {
        // Base: ConcertArtistUpdateInputFromConcert (will have create, insert, and update fields)
        // Nested: ConcertArtistUpdateInputFromConcertNested (will have the type fields to be updated)
        let base = Self::data_type_name(field_type_name, Some(&resolved_composite_type.name));
        let nested = format!("{}Nested", &base);
        vec![base, nested]
    }

    /// The field corresponding to the a one-to-many data parameter is different for update.
    /// Such a field needs three subfields:
    /// "create" to allow adding new items. The shape of this fields is the same as if it were a top-level field.
    /// "delete" to allow removing items. The shape of this fields is the same as if it were a top-level field (i.e. a reference type).
    /// "update" to allow updating items. The shape of this fields is the same as if it were a top-level field, except it also includes the "id" field.
    ///
    /// In this function we create four types. Three as described above, and one to include those three types. To differentiate the nested "update" type
    /// from the containing "update" type, we add a "Nested" suffix.
    fn expand_one_to_many(
        &self,
        _entity_type: &EntityType,
        field: &PostgresField<EntityType>,
        field_type: &EntityType,
        building: &SystemContextBuilding,
        top_level_type: Option<&EntityType>,
        container_type: Option<&EntityType>,
        expanding_one_to_many: bool,
    ) -> Result<Vec<(SerializableSlabIndex<MutationType>, MutationType)>, ModelBuildingError> {
        let existing_type_name =
            Self::data_type_name(&field_type.name, container_type.map(|t| t.name.as_str()));
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        // If not already expanded
        if building.mutation_types[existing_type_id].entity_id == SerializableSlabIndex::shallow() {
            let fields_info = vec![
                (
                    "create",
                    create_data_type_name(
                        field.typ.name(),
                        container_type.map(|t| t.name.as_str()),
                    ),
                ),
                (
                    "update",
                    update_data_type_name(
                        field.typ.name(),
                        container_type.map(|t| t.name.as_str()),
                    ) + "Nested",
                ),
                ("delete", field.typ.name().reference_type()),
            ];

            let fields = fields_info
                .into_iter()
                .map(|(name, field_type_name)| {
                    let plain_field_type = FieldType::Plain(PostgresFieldType {
                        type_id: TypeIndex::Composite(
                            building.mutation_types.get_id(&field_type_name).unwrap(),
                        ),
                        type_name: field_type_name,
                    });
                    PostgresField {
                        name: name.to_string(),
                        // The nested "create", "update", and "delete" fields are all optional that take a list.
                        typ: FieldType::Optional(Box::new(FieldType::List(Box::new(
                            plain_field_type,
                        )))),
                        access: field.access.clone(),
                        relation: field.relation.clone(),
                        default_value: field.default_value.clone(),
                        readonly: field.readonly,
                        type_validation: None,
                        doc_comments: None,
                    }
                })
                .collect();

            let mut types = vec![(
                existing_type_id,
                MutationType {
                    name: existing_type_name.clone(),
                    fields,
                    entity_id: building
                        .core_subsystem
                        .entity_types
                        .get_id(&field_type.name)
                        .unwrap(),
                    database_access: None,
                    doc_comments: field_type.doc_comments.clone(),
                },
            )];

            let nested_type = {
                let nested_existing_type_name = existing_type_name + "Nested";
                let nested_existing_type_id = building
                    .mutation_types
                    .get_id(&nested_existing_type_name)
                    .unwrap();

                let expanded_data_type = self.expanded_data_type(
                    field_type,
                    building,
                    top_level_type,
                    container_type,
                    expanding_one_to_many,
                )?;

                expanded_data_type.first().map(|tpe| {
                    let base_type = tpe.1.clone();
                    let mut base_type_fields = base_type.fields;

                    let base_type_pk_fields =
                        base_type_fields.iter_mut().filter(|f| f.relation.is_pk());

                    // For a non-nested type ("base type"), we already have the PK field, but it is optional. So here
                    // we make it required (by not wrapping the entity_pk_field it as optional)
                    for (i, base_type_pk_field) in base_type_pk_fields.enumerate() {
                        let entity_pk_field = field_type.pk_fields()[i];
                        base_type_pk_field.typ = to_mutation_type(
                            &entity_pk_field.typ,
                            MutationTypeKind::Update,
                            building,
                        );
                    }

                    let type_with_id = MutationType {
                        name: nested_existing_type_name,
                        fields: base_type_fields,
                        ..base_type
                    };

                    (nested_existing_type_id, type_with_id)
                })
            }
            .clone();

            if let Some(nested_type) = nested_type {
                types.push(nested_type);
            }

            Ok(types)
        } else {
            Ok(vec![])
        }
    }
}
