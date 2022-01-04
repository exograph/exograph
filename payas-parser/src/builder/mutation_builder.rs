//! Build mutation input types (<Type>CreationInput, <Type>UpdateInput, <Type>ReferenceInput) and
//! mutations (create<Type>, update<Type>, and delete<Type> as well as their plural versions)

use payas_model::model::access::Access;
use payas_model::model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_model::model::naming::ToGqlTypeNames;
use payas_model::model::operation::{Interceptors, Mutation, MutationKind, OperationReturnType};
use payas_model::model::relation::GqlRelation;
use payas_model::model::{
    GqlCompositeType, GqlField, GqlFieldType, GqlType, GqlTypeKind, GqlTypeModifier,
};

use super::create_mutation_builder::CreateMutationBuilder;
use super::delete_mutation_builder::DeleteMutationBuilder;
use super::reference_input_type_builder::ReferenceInputTypeBuilder;
use super::resolved_builder::{ResolvedCompositeType, ResolvedFieldType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::update_mutation_builder::UpdateMutationBuilder;

use super::Builder;

// TODO: Introduce this module as a struct (and have it hold the sub-builders)
// TODO: Abstract the concept of composite builders

/// Build shallow mutation input types
pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_shallow_only_persistent(models, building);

    CreateMutationBuilder {}.build_shallow_only_persistent(models, building);
    UpdateMutationBuilder {}.build_shallow_only_persistent(models, building);
    DeleteMutationBuilder {}.build_shallow_only_persistent(models, building);
}

/// Expand the mutation input types as well as build the mutation
pub fn build_expanded(building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_expanded(building); // Used by many...

    CreateMutationBuilder {}.build_expanded(building);
    UpdateMutationBuilder {}.build_expanded(building);
    DeleteMutationBuilder {}.build_expanded(building);
}

pub trait MutationBuilder {
    fn single_mutation_name(model_type: &GqlType) -> String;
    fn single_mutation_kind(model_type: &GqlType, building: &SystemContextBuilding)
        -> MutationKind;

    fn multi_mutation_name(model_type: &GqlType) -> String;
    fn multi_mutation_kind(model_type: &GqlType, building: &SystemContextBuilding) -> MutationKind;

    fn build_mutations(
        &self,
        model_type_id: SerializableSlabIndex<GqlType>,
        model_type: &GqlType,
        building: &SystemContextBuilding,
    ) -> Vec<Mutation> {
        let single_mutation = Mutation {
            name: Self::single_mutation_name(model_type),
            kind: Self::single_mutation_kind(model_type, building),
            return_type: OperationReturnType {
                type_id: model_type_id,
                type_name: model_type.name.clone(),
                type_modifier: GqlTypeModifier::Optional,
            },
            interceptors: Interceptors::default(),
        };

        let multi_mutation = Mutation {
            name: Self::multi_mutation_name(model_type),
            kind: Self::multi_mutation_kind(model_type, building),
            return_type: OperationReturnType {
                type_id: model_type_id,
                type_name: model_type.name.clone(),
                type_modifier: GqlTypeModifier::List,
            },
            interceptors: Interceptors::default(),
        };

        vec![single_mutation, multi_mutation]
    }
}

pub trait DataParamBuilder<D> {
    fn data_param_type_name(resolved_composite_type: &ResolvedCompositeType) -> String {
        Self::base_data_type_name(&resolved_composite_type.name)
    }

    fn base_data_type_name(model_type_name: &str) -> String;

    fn data_param(model_type: &GqlType, building: &SystemContextBuilding, array: bool) -> D;

    fn data_type_name(model_type_name: &str, container_type: Option<&str>) -> String {
        let base_name = Self::base_data_type_name(model_type_name);
        match container_type {
            Some(container_type) => {
                format!("{}From{}", base_name, container_type)
            }
            None => base_name,
        }
    }

    fn compute_data_fields(
        &self,
        gql_fields: &[GqlField],
        top_level_type: Option<&GqlType>,
        container_type: Option<&str>,
        building: &SystemContextBuilding,
    ) -> Vec<GqlField> {
        gql_fields
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
                    ResolvedFieldType::Optional(inner_type) => inner_type.as_ref(),
                    _ => &field.typ,
                };

                if let ResolvedFieldType::List(_) = typ {
                    if let ResolvedType::Composite(ResolvedCompositeType { name, .. }) =
                        typ.deref(resolved_types)
                    {
                        Self::data_param_field_one_to_many_type_names(name, resolved_composite_type)
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
        field: &GqlField,
        top_level_type: Option<&GqlType>,
        container_type: Option<&str>,
        building: &SystemContextBuilding,
    ) -> Option<GqlField> {
        let optional = Self::mark_fields_optional();

        match &field.relation {
            GqlRelation::Pk { .. } => None, // TODO: Make this decision based on autoincrement/uuid etc of the id
            GqlRelation::Scalar { .. } | GqlRelation::NonPersistent => Some(GqlField {
                typ: if optional {
                    field.typ.optional()
                } else {
                    field.typ.clone()
                },
                ..field.clone()
            }),
            GqlRelation::OneToMany { .. } => {
                self.compute_one_to_many_data_field(field, container_type, building)
            }
            GqlRelation::ManyToOne { .. } => {
                let field_type_name = field.typ.type_name().reference_type();
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = GqlFieldType::Reference {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = match field.typ {
                    GqlFieldType::Reference { .. } => {
                        if optional {
                            field_plain_type.optional()
                        } else {
                            field_plain_type
                        }
                    }
                    GqlFieldType::Optional(_) => GqlFieldType::Optional(Box::new(field_plain_type)),
                    GqlFieldType::List(_) => GqlFieldType::List(Box::new(field_plain_type)),
                };

                match &top_level_type {
                    Some(value) if value.name == field.typ.type_name() => None,
                    _ => Some(GqlField {
                        name: field.name.clone(),
                        typ: field_type,
                        relation: field.relation.clone(),
                    }),
                }
            }
        }
    }

    fn compute_one_to_many_data_field(
        &self,
        field: &GqlField,
        container_type: Option<&str>,
        building: &SystemContextBuilding,
    ) -> Option<GqlField> {
        let optional = Self::mark_fields_optional();

        let field_type_name = Self::data_type_name(field.typ.type_name(), container_type);

        building
            .mutation_types
            .get_id(&field_type_name)
            .and_then(|field_type_id| {
                let field_plain_type = GqlFieldType::Reference {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = GqlFieldType::List(Box::new(field_plain_type));

                match &container_type {
                    Some(value) if value == &field.typ.type_name() => None,
                    _ => Some(GqlField {
                        name: field.name.clone(),
                        typ: if optional {
                            field_type.optional()
                        } else {
                            field_type
                        },
                        relation: field.relation.clone(),
                    }),
                }
            })
    }

    fn expanded_data_type(
        &self,
        model_type: &GqlType,
        building: &SystemContextBuilding,
        top_level_type: Option<&GqlType>,
        container_type: Option<&GqlType>,
    ) -> Vec<(SerializableSlabIndex<GqlType>, GqlCompositeType)> {
        if let GqlTypeKind::Composite(GqlCompositeType {
            ref fields, kind, ..
        }) = &model_type.kind
        {
            let model_fields = fields;

            let mut field_types: Vec<_> = model_fields
                .iter()
                .flat_map(|field| {
                    let field_type = field.typ.base_type(&building.types.values);
                    if let (GqlTypeKind::Composite(_), GqlFieldType::List(_)) =
                        (&field_type.kind, &field.typ)
                    {
                        self.expand_one_to_many(
                            model_type,
                            field,
                            field_type,
                            building,
                            top_level_type,
                            Some(model_type),
                        )
                    } else {
                        vec![]
                    }
                })
                .collect();

            let existing_type_name = Self::data_type_name(
                model_type.name.as_str(),
                container_type.map(|value| value.name.as_str()),
            );
            let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

            let input_type_fields = self.compute_data_fields(
                model_fields,
                top_level_type,
                Some(model_type.name.as_str()),
                building,
            );
            field_types.push((
                existing_type_id,
                GqlCompositeType {
                    fields: input_type_fields,
                    kind: kind.clone(),
                    access: Access::restrictive(),
                },
            ));

            field_types
        } else {
            vec![]
        }
    }

    fn expand_one_to_many(
        &self,
        model_type: &GqlType,
        _field: &GqlField,
        field_type: &GqlType,
        building: &SystemContextBuilding,
        top_level_type: Option<&GqlType>,
        _container_type: Option<&GqlType>,
    ) -> Vec<(SerializableSlabIndex<GqlType>, GqlCompositeType)> {
        let new_container_type = Some(model_type);

        let existing_type_name = Self::data_type_name(
            &field_type.name,
            new_container_type.map(|value| value.name.as_str()),
        );

        if let GqlTypeKind::Primitive = building
            .mutation_types
            .get_by_key(&existing_type_name)
            .unwrap()
            .kind
        {
            // If not already expanded (i.e. the kind is primitive)
            self.expanded_data_type(field_type, building, top_level_type, new_container_type)
        } else {
            vec![]
        }
    }
}

pub fn create_data_type_name(model_type_name: &str, container_type: &Option<&str>) -> String {
    let base_name = model_type_name.creation_type();
    data_type_name(base_name, container_type)
}

pub fn update_data_type_name(model_type_name: &str, container_type: &Option<&str>) -> String {
    let base_name = model_type_name.update_type();
    data_type_name(base_name, container_type)
}

fn data_type_name(base_name: String, container_type: &Option<&str>) -> String {
    match container_type {
        Some(container_type) => {
            format!("{}From{}", base_name, container_type)
        }
        None => base_name,
    }
}
