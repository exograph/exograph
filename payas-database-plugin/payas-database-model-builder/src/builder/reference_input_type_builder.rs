//! Build the reference input type (used to refer to an entity by its pk)

use payas_core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_database_model::access::Access;
use payas_database_model::relation::DatabaseRelation;
use payas_database_model::types::{DatabaseCompositeType, DatabaseType, DatabaseTypeKind};

use super::naming::ToDatabaseTypeNames;

use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::type_builder::ResolvedTypeEnv;
use super::Builder;

pub struct ReferenceInputTypeBuilder;

impl Builder for ReferenceInputTypeBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        _models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        vec![resolved_composite_type.reference_type()]
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(
        &self,
        _resolved_env: &ResolvedTypeEnv,
        building: &mut SystemContextBuilding,
    ) {
        for (_, model_type) in building.database_types.iter() {
            if let DatabaseTypeKind::Composite(DatabaseCompositeType { .. }) = &model_type.kind {
                for (existing_id, expanded_kind) in expanded_reference_types(model_type, building) {
                    building.mutation_types[existing_id].kind = expanded_kind;
                }
            }
        }
    }
}

fn expanded_reference_types(
    model_type: &DatabaseType,
    building: &SystemContextBuilding,
) -> Vec<(SerializableSlabIndex<DatabaseType>, DatabaseTypeKind)> {
    let existing_type = model_type;

    if let DatabaseTypeKind::Composite(DatabaseCompositeType {
        ref fields,
        pk_query,
        collection_query,
        table_id,
        ..
    }) = &existing_type.kind
    {
        let reference_type_fields = fields
            .clone()
            .into_iter()
            .flat_map(|field| match &field.relation {
                DatabaseRelation::Pk { .. } => Some(field),
                _ => None,
            })
            .collect();

        let existing_type_name = model_type.reference_type();
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        vec![(
            existing_type_id,
            DatabaseTypeKind::Composite(DatabaseCompositeType {
                fields: reference_type_fields,
                pk_query: *pk_query,
                collection_query: *collection_query,
                table_id: *table_id,
                access: Access::restrictive(),
            }),
        )]
    } else {
        vec![]
    }
}
