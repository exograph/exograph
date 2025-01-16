// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Build mutation input types associated with creation (`<Type>CreationInput`) and
//! the create mutations (`create<Type>`, and `create<Type>s`)

use core_plugin_interface::{
    core_model::{
        access::AccessPredicateExpression,
        mapped_arena::MappedArena,
        types::{BaseOperationReturnType, FieldType, OperationReturnType},
    },
    core_model_builder::error::ModelBuildingError,
};

use postgres_core_model::types::EntityType;
use postgres_graphql_model::mutation::{
    DataParameter, DataParameterType, PostgresMutationParameters,
};

use crate::mutation_builder::DataParamRole;

use super::{
    builder::Builder,
    mutation_builder::{DataParamBuilder, MutationBuilder},
    naming::ToPostgresMutationNames,
    naming::ToPostgresTypeNames,
    system_builder::SystemContextBuilding,
};

use postgres_core_builder::resolved_type::ResolvedCompositeType;
use postgres_core_builder::resolved_type::ResolvedType;

pub struct CreateMutationBuilder;

impl Builder for CreateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        if !resolved_composite_type.access.creation_allowed() {
            return vec![];
        }
        let mut field_types = self.data_param_field_type_names(resolved_composite_type, types);
        field_types.push(Self::data_param_type_name(resolved_composite_type));
        field_types
    }

    fn build_expanded(
        &self,
        building: &mut SystemContextBuilding,
    ) -> Result<(), ModelBuildingError> {
        let creation_access_is_false = |entity_type: &EntityType| -> bool {
            matches!(
                building
                    .core_subsystem
                    .input_access_expressions
                    .lock()
                    .unwrap()[entity_type.access.creation.input],
                AccessPredicateExpression::BooleanLiteral(false)
            ) || matches!(
                building
                    .core_subsystem
                    .precheck_access_expressions
                    .lock()
                    .unwrap()[entity_type.access.creation.pre_creation],
                AccessPredicateExpression::BooleanLiteral(false)
            )
        };

        for (_, entity_type) in building.core_subsystem.entity_types.iter() {
            if !creation_access_is_false(entity_type) {
                for (existing_id, expanded_type) in
                    self.expanded_data_type(entity_type, building, Some(entity_type), None, false)?
                {
                    building.mutation_types[existing_id] = expanded_type;
                }
            }
        }

        for (_, entity_type) in building.core_subsystem.entity_types.iter() {
            if !creation_access_is_false(entity_type) {
                let entity_type_id = building
                    .core_subsystem
                    .entity_types
                    .get_id(entity_type.name.as_str())
                    .unwrap();

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

impl MutationBuilder for CreateMutationBuilder {
    fn single_mutation_name(entity_type: &EntityType) -> String {
        entity_type.pk_create()
    }

    fn single_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters {
        PostgresMutationParameters::Create(Self::data_param(entity_type, building, false))
    }

    fn single_mutation_modified_type(
        base_type: BaseOperationReturnType<EntityType>,
    ) -> OperationReturnType<EntityType> {
        OperationReturnType::Plain(base_type)
    }

    fn multi_mutation_name(entity_type: &EntityType) -> String {
        entity_type.collection_create()
    }

    fn multi_mutation_parameters(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationParameters {
        PostgresMutationParameters::Create(Self::data_param(entity_type, building, true))
    }
}

impl DataParamBuilder<DataParameter> for CreateMutationBuilder {
    fn mark_fields_optional() -> bool {
        false
    }

    fn use_list_for_nested_one_to_many() -> bool {
        true
    }

    fn base_data_type_name(entity_type_name: &str) -> String {
        entity_type_name.creation_type()
    }

    fn data_param_role() -> DataParamRole {
        DataParamRole::Create
    }

    fn data_param(
        entity_type: &EntityType,
        building: &SystemContextBuilding,
        array: bool,
    ) -> DataParameter {
        let data_param_type_name = Self::base_data_type_name(&entity_type.name);
        let data_param_type_id = building
            .mutation_types
            .get_id(&data_param_type_name)
            .unwrap();

        let base_type = FieldType::Plain(DataParameterType {
            name: data_param_type_name,
            type_id: data_param_type_id,
            type_validation: None,
        });

        DataParameter {
            name: "data".to_string(),
            typ: if array {
                FieldType::List(Box::new(base_type))
            } else {
                base_type
            },
            type_validation: None,
        }
    }
}
