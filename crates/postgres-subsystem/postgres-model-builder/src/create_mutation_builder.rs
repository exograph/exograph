//! Build mutation input types associated with creation (<Type>CreationInput) and
//! the create mutations (create<Type>, and create<Type>s)

use core_plugin_interface::core_model::mapped_arena::{MappedArena, SerializableSlabIndex};

use postgres_model::{
    operation::{CreateDataParameter, CreateDataParameterTypeWithModifier, PostgresMutationKind},
    types::{EntityType, PostgresTypeModifier},
};

use crate::mutation_builder::DataParamRole;

use super::{
    builder::Builder,
    mutation_builder::{DataParamBuilder, MutationBuilder},
    naming::{ToPostgresMutationNames, ToPostgresTypeNames},
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
};

pub struct CreateMutationBuilder;

impl Builder for CreateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        let mut field_types = self.data_param_field_type_names(resolved_composite_type, types);
        field_types.push(Self::data_param_type_name(resolved_composite_type));
        field_types
    }

    fn build_expanded(&self, resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
        for (_, entity_type) in building.entity_types.iter() {
            for (existing_id, expanded_type) in self.expanded_data_type(
                entity_type,
                resolved_env,
                building,
                Some(entity_type),
                None,
            ) {
                building.mutation_types[existing_id] = expanded_type;
            }
        }

        for (_, entity_type) in building.entity_types.iter() {
            let entity_type_id = building
                .entity_types
                .get_id(entity_type.name.as_str())
                .unwrap();

            for mutation in self.build_mutations(entity_type_id, entity_type, building) {
                building.mutations.add(&mutation.name.to_owned(), mutation);
            }
        }
    }
}

impl MutationBuilder for CreateMutationBuilder {
    fn single_mutation_name(entity_type: &EntityType) -> String {
        entity_type.pk_create()
    }

    fn single_mutation_kind(
        _entity_type_id: SerializableSlabIndex<EntityType>,
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationKind {
        PostgresMutationKind::Create(Self::data_param(entity_type, building, false))
    }

    fn single_mutation_type_modifier() -> PostgresTypeModifier {
        PostgresTypeModifier::NonNull
    }

    fn multi_mutation_name(entity_type: &EntityType) -> String {
        entity_type.collection_create()
    }

    fn multi_mutation_kind(
        _entity_type_id: SerializableSlabIndex<EntityType>,
        entity_type: &EntityType,
        building: &SystemContextBuilding,
    ) -> PostgresMutationKind {
        PostgresMutationKind::Create(Self::data_param(entity_type, building, true))
    }
}

impl DataParamBuilder<CreateDataParameter> for CreateMutationBuilder {
    fn mark_fields_optional() -> bool {
        false
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
    ) -> CreateDataParameter {
        let data_param_type_name = Self::base_data_type_name(&entity_type.name);
        let data_param_type_id = building
            .mutation_types
            .get_id(&data_param_type_name)
            .unwrap();

        CreateDataParameter {
            name: "data".to_string(),
            typ: CreateDataParameterTypeWithModifier {
                type_name: data_param_type_name,
                type_id: data_param_type_id,
                array_input: array,
            },
        }
    }
}
