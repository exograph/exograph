//! Build update mutation types <Type>UpdateInput, update<Type>, and update<Type>s

use payas_model::model::access::Access;
use payas_model::model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_model::model::naming::{ToGqlMutationNames, ToGqlTypeNames};
use payas_model::model::types::GqlType;
use payas_model::model::{
    GqlCompositeKind, GqlCompositeTypeKind, GqlField, GqlFieldType, GqlTypeKind,
};

use crate::builder::mutation_builder::{create_data_type_name, update_data_type_name};
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
        field_types.push(Self::data_param_type_name(resolved_composite_type));
        field_types
    }

    /// Expand the mutation input types as well as build the mutation
    fn build_expanded(&self, building: &mut SystemContextBuilding) {
        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite(GqlCompositeTypeKind {
                kind: GqlCompositeKind::Persistent { .. },
                ..
            }) = &model_type.kind
            {
                for (existing_id, expanded_kind) in
                    self.expanded_data_type(model_type, building, Some(model_type), None)
                {
                    building.mutation_types[existing_id].kind =
                        GqlTypeKind::Composite(expanded_kind);
                }
            }
        }

        for (_, model_type) in building.types.iter() {
            if let GqlTypeKind::Composite(GqlCompositeTypeKind {
                kind: GqlCompositeKind::Persistent { .. },
                ..
            }) = &model_type.kind
            {
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
    // from the containing "update" type, we add a "Nested" suffix.
    fn expand_one_to_many(
        &self,
        model_type: &GqlType,
        field: &GqlField,
        field_type: &GqlType,
        building: &SystemContextBuilding,
        top_level_type: Option<&GqlType>,
        container_type: Option<&GqlType>,
    ) -> Vec<(SerializableSlabIndex<GqlType>, GqlCompositeTypeKind)> {
        let existing_type_name =
            Self::data_type_name(&field_type.name, container_type.map(|t| t.name.as_str()));
        let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

        let nested_type = {
            let nested_existing_type_id = {
                let nested_existing_type_name =
                    Self::data_type_name(&field_type.name, container_type.map(|t| t.name.as_str()))
                        + "Nested";
                building
                    .mutation_types
                    .get_id(&nested_existing_type_name)
                    .unwrap()
            };

            &self
                .expanded_data_type(field_type, building, top_level_type, container_type)
                .first()
                .map(|tpe| {
                    let base_type = tpe.1.clone();
                    let mut fields_with_id = Vec::with_capacity(base_type.fields.len() + 1);

                    let model_pk_field = model_type.pk_field().unwrap();

                    let update_pk_field = GqlField {
                        typ: model_pk_field.typ.clone(),
                        ..model_pk_field.clone()
                    };
                    fields_with_id.push(update_pk_field);

                    fields_with_id.extend(base_type.fields.into_iter());

                    let type_with_id = GqlCompositeTypeKind {
                        fields: fields_with_id,
                        ..base_type
                    };

                    (nested_existing_type_id, type_with_id)
                })
        }
        .clone();

        if let GqlTypeKind::Composite(GqlCompositeTypeKind {
            kind: kind @ GqlCompositeKind::Persistent { .. },
            ..
        }) = &model_type.kind
        {
            // If not already expanded (i.e. the kind is primitive)
            if let GqlTypeKind::Primitive = building.mutation_types[existing_type_id].kind {
                let fields_info = vec![
                    (
                        "create",
                        create_data_type_name(
                            field.typ.type_name(),
                            &container_type.map(|t| t.name.as_str()),
                        ),
                    ),
                    (
                        "update",
                        update_data_type_name(
                            field.typ.type_name(),
                            &container_type.map(|t| t.name.as_str()),
                        ) + "Nested",
                    ),
                    ("delete", field.typ.type_name().reference_type()),
                ];

                let fields = fields_info
                    .into_iter()
                    .map(|(name, field_type_name)| {
                        let plain_field_type = GqlFieldType::Reference {
                            type_id: building.mutation_types.get_id(&field_type_name).unwrap(),
                            type_name: field_type_name,
                        };
                        GqlField {
                            name: String::from(name),
                            typ: GqlFieldType::List(Box::new(plain_field_type)),
                            relation: field.relation.clone(),
                        }
                    })
                    .collect();
                let mut types = vec![(
                    existing_type_id,
                    GqlCompositeTypeKind {
                        fields,
                        kind: kind.clone(),
                        access: Access::restrictive(),
                    },
                )];

                if let Some(nested_type) = nested_type {
                    types.push(nested_type);
                }

                types
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
}
