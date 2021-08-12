//! Build update mutation typ name: (), typ: (), relation: () es <Type>UpdateInput, update<Type>, and update<Type>s fields: (), table_id: (), pk_query: (), collection_query: (), access: ()  fields: (), table_id: (), pk_query: (), collection_query: (), access: ()

use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::types::GqlType;
use payas_model::model::GqlTypeKind;

use crate::builder::query_builder;

use payas_model::model::operation::{MutationKind, UpdateDataParameter};

use super::mutation_builder::{DataParamBuilder, MutationBuilder};
use super::resolved_builder::{ResolvedCompositeType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::Builder;

pub struct UpdateMutationBuilder;

impl Builder for UpdateMutationBuilder {
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        // TODO: This implementation is the same for CreateMutationBuilder. Fix it when we refactor non-mutations builders
        let mut field_types = self.data_param_field_type_names(resolved_composite_type, models);
        field_types.push(self.data_param_type_name(resolved_composite_type));
        field_types
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                for (existing_id, expanded_kind) in
                    self.expanded_data_type(model_type, building, Some(&model_type.name), None)
                {
                    building.mutation_types[existing_id].kind = expanded_kind;
                }
            }
        }

        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite { .. } = &model_type.kind {
                let model_type_id = building.types.get_id(model_type.name.as_str()).unwrap();

                for mutation in self.build_mutations(model_type_id, model_type, building) {
                    building.mutations.add(&mutation.name.to_owned(), mutation);
                }
            }
        }
    }
}

impl MutationBuilder for UpdateMutationBuilder {
    fn single_mutation_name(model_type: &GqlType) -> String {
        model_type.pk_update()
    }

    fn single_mutation_kind(
        model_type: &GqlType,
        building: &SystemContextBuilding,
    ) -> MutationKind {
        MutationKind::Update {
            data_param: Self::data_param(model_type, building, false),
            predicate_param: query_builder::pk_predicate_param(model_type, building),
        }
    }

    fn multi_mutation_name(model_type: &GqlType) -> String {
        model_type.collection_update()
    }

    fn multi_mutation_kind(model_type: &GqlType, building: &SystemContextBuilding) -> MutationKind {
        MutationKind::Update {
            data_param: Self::data_param(model_type, building, true),
            predicate_param: query_builder::collection_predicate_param(model_type, building),
        }
    }
}

impl DataParamBuilder<UpdateDataParameter> for UpdateMutationBuilder {
    fn mark_fields_optional() -> bool {
        true
    }

    fn base_data_type_name(model_type_name: &str) -> String {
        model_type_name.update_type()
    }

    fn data_param(
        model_type: &GqlType,
        building: &SystemContextBuilding,
        _array: bool,
    ) -> UpdateDataParameter {
        let data_param_type_name = Self::base_data_type_name(&model_type.name);
        let data_param_type_id = building
            .mutation_types
            .get_id(&data_param_type_name)
            .unwrap();

        UpdateDataParameter {
            name: "data".to_string(),
            type_name: data_param_type_name,
            type_id: data_param_type_id,
        }
    }

    fn data_param_field_one_to_many_type_names(
        field_type_name: &str,
        resolved_composite_type: &ResolvedCompositeType,
    ) -> Vec<String> {
        // Base: ConcertArtistUpdateInputFromConcert (will have create, insert, and update fields)
        // Nested: ConcertArtistUpdateInputFromConcertNested (will have the model fields to be updated)
        let base = Self::data_type_name(field_type_name, &Some(&resolved_composite_type.name));
        let nested = format!("{}Nested", &base);
        vec![base, nested]
    }
    /*
    fn expand_one_to_many(
        &self,
        model_type: &GqlType,
        field: &GqlField,
        field_type: &GqlType,
        building: &SystemContextBuilding,
        container_types: &[&str],
        _new_container_types: &[&str],
    ) -> Vec<(Id<GqlType>, GqlTypeKind)> {
        let existing_type_name =
            Self::data_type_name(&field_type.name, container_types.first().copied());
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        dbg!(&existing_type_name, container_types, _new_container_types);
        if let GqlTypeKind::Composite(GqlCompositeTypeKind {
            table_id,
            pk_query,
            collection_query,
            ..
        }) = model_type.kind
        {
            println!("Creating {}", &existing_type_name);
            // If not already expanded (i.e. the kind is primitive)
            if let GqlTypeKind::Primitive = building.mutation_types[existing_type_id].kind {
                vec![(
                    existing_type_id,
                    GqlTypeKind::Composite(GqlCompositeTypeKind {
                        fields: vec![GqlField {
                            name: String::from("create"),
                            typ: field.typ.clone(),
                            relation: field.relation.clone(),
                        }],
                        table_id: table_id,
                        pk_query: pk_query,
                        collection_query: collection_query,
                        access: Access::restrictive(),
                    }),
                )]
                //self.expanded_data_type(field_type, building, new_container_types.to_owned())
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }*/
}
