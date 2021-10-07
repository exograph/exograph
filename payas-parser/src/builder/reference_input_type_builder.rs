//! Build the reference input type (used to refer to an entity by its pk)

use payas_model::model::access::Access;
use payas_model::model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_model::model::naming::ToGqlTypeNames;
use payas_model::model::relation::GqlRelation;
use payas_model::model::types::GqlType;
use payas_model::model::{GqlCompositeTypeKind, GqlTypeKind};

use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;
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
    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                for (existing_id, expanded_kind) in expanded_reference_types(model_type, building) {
                    building.mutation_types[existing_id].kind = expanded_kind;
                }
            }
        }
    }
}

fn expanded_reference_types(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Vec<(SerializableSlabIndex<GqlType>, GqlTypeKind)> {
    let existing_type = model_type;

    if let GqlTypeKind::Composite(GqlCompositeTypeKind {
        ref fields, kind, ..
    }) = &existing_type.kind
    {
        let reference_type_fields = fields
            .clone()
            .into_iter()
            .flat_map(|field| match &field.relation {
                GqlRelation::Pk { .. } => Some(field),
                _ => None,
            })
            .collect();

        let existing_type_name = model_type.reference_type();
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        vec![(
            existing_type_id,
            GqlTypeKind::Composite(GqlCompositeTypeKind {
                fields: reference_type_fields,
                kind: kind.clone(),
                access: Access::restrictive(),
            }),
        )]
    } else {
        vec![]
    }
}
