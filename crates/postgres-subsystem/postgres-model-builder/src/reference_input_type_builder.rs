//! Build the reference input type (used to refer to an entity by its pk)

use core_plugin_interface::core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use postgres_model::{access::Access, relation::PostgresRelation, types::PostgresCompositeType};

use super::{
    builder::Builder,
    naming::ToPostgresTypeNames,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
};

pub struct ReferenceInputTypeBuilder;

impl Builder for ReferenceInputTypeBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        _types: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        vec![resolved_composite_type.reference_type()]
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(
        &self,
        _resolved_env: &ResolvedTypeEnv,
        building: &mut SystemContextBuilding,
    ) {
        for (_, model_type) in building.entity_types.iter() {
            for (existing_id, expanded_type) in expanded_reference_types(model_type, building) {
                building.mutation_types[existing_id] = expanded_type;
            }
        }
    }
}

fn expanded_reference_types(
    model_type: &PostgresCompositeType,
    building: &SystemContextBuilding,
) -> Vec<(
    SerializableSlabIndex<PostgresCompositeType>,
    PostgresCompositeType,
)> {
    let existing_type = model_type;

    let PostgresCompositeType {
        ref fields,
        pk_query,
        collection_query,
        aggregate_query,
        table_id,
        ..
    } = &existing_type;

    {
        let reference_type_fields = fields
            .clone()
            .into_iter()
            .flat_map(|field| match &field.relation {
                PostgresRelation::Pk { .. } => Some(field),
                _ => None,
            })
            .collect();

        let existing_type_name = model_type.reference_type();
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        vec![(
            existing_type_id,
            PostgresCompositeType {
                name: existing_type_name,
                plural_name: "".to_owned(),
                fields: reference_type_fields,
                agg_fields: vec![],
                pk_query: *pk_query,
                collection_query: *collection_query,
                aggregate_query: *aggregate_query,
                table_id: *table_id,
                access: Access::restrictive(),
                is_input: true,
            },
        )]
    }
}
