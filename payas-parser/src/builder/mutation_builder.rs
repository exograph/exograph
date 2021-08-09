//! Build mutation input types (<Type>CreationInput, <Type>UpdateInput, <Type>ReferenceInput) and
//! mutations (create<Type>, update<Type>, and delete<Type> as well as their plural versions)

use std::collections::HashSet;

use id_arena::Id;
use payas_model::model::access::Access;
use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::ToGqlTypeNames;
use payas_model::model::operation::{Mutation, MutationKind, OperationReturnType};
use payas_model::model::relation::GqlRelation;
use payas_model::model::{
    GqlCompositeTypeKind, GqlField, GqlFieldType, GqlType, GqlTypeKind, GqlTypeModifier,
};

use super::create_mutation_builder::CreateMutationBuilder;
use super::delete_mutation_builder::DeleteMutationBuilder;
use super::reference_input_type_builder::ReferenceInputTypeBuilder;
use super::resolved_builder::{ResolvedCompositeType, ResolvedFieldType, ResolvedType};
use super::system_builder::SystemContextBuilding;
use super::update_mutation_builder::UpdateMutationBuilder;

use super::builder::Builder;

// TODO: Introduce this as a struct (and have it hold the sub-builders)
// TODO: Abstract the concept of compisite builders

/// Build shallow mutaiton input types
pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_shallow(models, building);

    CreateMutationBuilder {}.build_shallow(models, building);
    UpdateMutationBuilder {}.build_shallow(models, building);
    DeleteMutationBuilder {}.build_shallow(models, building);
}

/// Expand the mutation input types as well as build the mutation
pub fn build_expanded(building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_expanded(building); // Used by many...

    CreateMutationBuilder {}.build_expanded(building);
    UpdateMutationBuilder {}.build_expanded(building);
    DeleteMutationBuilder {}.build_expanded(building);
}

pub trait CreateUpdateBuilder {
    fn input_type_name(model_type_name: &str, container_type: Option<&str>) -> String {
        let base_name = Self::base_input_type_name(model_type_name);
        match container_type {
            Some(container_type) => {
                format!("{}From{}", base_name, container_type)
            }
            None => base_name,
        }
    }

    fn single_mutation_name(model_type: &GqlType) -> String;
    fn single_mutation_kind(
        model_type: &GqlType,
        param_type_name: &str,
        param_type_id: Id<GqlType>,
        building: &SystemContextBuilding,
    ) -> MutationKind;

    fn multi_mutation_name(model_type: &GqlType) -> String;
    fn multi_mutation_kind(
        model_type: &GqlType,
        param_type_name: &str,
        param_type_id: Id<GqlType>,
        building: &SystemContextBuilding,
    ) -> MutationKind;

    fn base_input_type_name(model_type_name: &str) -> String;

    fn build_mutations(
        &self,
        model_type_id: Id<GqlType>,
        model_type: &GqlType,
        building: &SystemContextBuilding,
    ) -> Vec<Mutation> {
        let data_param_type_name = Self::base_input_type_name(&model_type.name);
        let data_param_type_id = building
            .mutation_types
            .get_id(&data_param_type_name)
            .unwrap();

        let single_mutation = Mutation {
            name: Self::single_mutation_name(model_type),
            kind: Self::single_mutation_kind(
                model_type,
                &data_param_type_name,
                data_param_type_id,
                building,
            ),
            return_type: OperationReturnType {
                type_id: model_type_id,
                type_name: model_type.name.clone(),
                type_modifier: GqlTypeModifier::Optional,
            },
        };

        let multi_mutation = Mutation {
            name: Self::multi_mutation_name(model_type),
            kind: Self::multi_mutation_kind(
                model_type,
                &data_param_type_name,
                data_param_type_id,
                building,
            ),
            return_type: OperationReturnType {
                type_id: model_type_id,
                type_name: model_type.name.clone(),
                type_modifier: GqlTypeModifier::List,
            },
        };

        vec![single_mutation, multi_mutation]
    }

    // TODO: Revisit this after nested update mutation works
    fn mark_fields_optional() -> bool;

    fn compute_input_field(
        &self,
        field: &GqlField,
        container_types: &Vec<&str>,
        building: &SystemContextBuilding,
    ) -> Option<GqlField> {
        let optional = Self::mark_fields_optional();

        match &field.relation {
            GqlRelation::Pk { .. } => None, // TODO: Make this decistion based on autoincrement/uuid etc of the id
            GqlRelation::Scalar { .. } => Some(GqlField {
                typ: if optional {
                    field.typ.optional()
                } else {
                    field.typ.clone()
                },
                ..field.clone()
            }),
            GqlRelation::OneToMany { .. } => {
                let field_type_name =
                    Self::input_type_name(field.typ.type_name(), container_types.first().copied());
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = GqlFieldType::Reference {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = GqlFieldType::List(Box::new(field_plain_type));

                if container_types.contains(&field.typ.type_name()) {
                    None
                } else {
                    Some(GqlField {
                        name: field.name.clone(),
                        typ: if optional {
                            field_type.optional()
                        } else {
                            field_type
                        },
                        relation: field.relation.clone(),
                    })
                }
            }
            GqlRelation::ManyToOne { .. } => {
                let field_type_name = field.typ.type_name().reference_type();
                let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
                let field_plain_type = GqlFieldType::Reference {
                    type_name: field_type_name,
                    type_id: field_type_id,
                };
                let field_type = match field.typ {
                    GqlFieldType::Reference { .. } => field_plain_type,
                    GqlFieldType::Optional(_) => GqlFieldType::Optional(Box::new(field_plain_type)),
                    GqlFieldType::List(_) => GqlFieldType::List(Box::new(field_plain_type)),
                };

                if container_types.contains(&field.typ.type_name()) {
                    None
                } else {
                    Some(GqlField {
                        name: field.name.clone(),
                        typ: field_type,
                        relation: field.relation.clone(),
                    })
                }
            }
        }
    }

    fn compute_input_fields(
        &self,
        gql_fields: &[GqlField],
        container_types: Vec<&str>,
        building: &SystemContextBuilding,
    ) -> Vec<GqlField> {
        gql_fields
            .iter()
            .flat_map(|field| self.compute_input_field(field, &container_types, building))
            .collect()
    }

    fn expanded_type(
        &self,
        model_type: &GqlType,
        building: &SystemContextBuilding,
        container_types: Vec<&str>,
        expanded_nested_mutation_types: &mut HashSet<String>,
    ) -> Vec<(Id<GqlType>, GqlTypeKind)> {
        if let GqlTypeKind::Composite(GqlCompositeTypeKind {
            ref fields,
            table_id,
            pk_query,
            collection_query,
            ..
        }) = model_type.kind
        {
            let model_fields = fields;

            let mut new_container_types = container_types.clone();
            new_container_types.push(&model_type.name);

            let mut creation_types: Vec<_> = model_fields
                .iter()
                .flat_map(|field| {
                    let field_type = field.typ.base_type(&building.types.values);
                    if let (GqlTypeKind::Composite(_), GqlFieldType::List(_)) =
                        (&field_type.kind, &field.typ)
                    {
                        let existing_type_name = Self::input_type_name(
                            &field_type.name,
                            container_types.first().copied(),
                        );

                        // Protect against going into an infinite loop when cycles are present
                        if !expanded_nested_mutation_types.contains(&existing_type_name) {
                            expanded_nested_mutation_types.insert(existing_type_name);
                            self.expanded_type(
                                field_type,
                                building,
                                new_container_types.clone(),
                                expanded_nested_mutation_types,
                            )
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                })
                .collect();

            let existing_type_name =
                Self::input_type_name(model_type.name.as_str(), container_types.first().copied());
            let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

            let input_type_fields =
                self.compute_input_fields(model_fields, new_container_types, building);
            creation_types.push((
                existing_type_id,
                GqlTypeKind::Composite(GqlCompositeTypeKind {
                    fields: input_type_fields,
                    table_id,
                    pk_query,
                    collection_query,
                    access: Access::restrictive(),
                }),
            ));

            creation_types
        } else {
            vec![]
        }
    }

    fn field_type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        models: &MappedArena<ResolvedType>,
    ) -> Vec<String> {
        resolved_composite_type
            .fields
            .iter()
            .filter_map(|field| {
                // Create a nested input data type only if it refers to a many side
                // So for Venue <-> [Concert] case, create only ConcertCreationInputFromVenue
                if let ResolvedFieldType::List(_) = field.typ {
                    if let ResolvedType::Composite(ResolvedCompositeType { name, .. }) =
                        field.typ.deref(models)
                    {
                        Some(Self::input_type_name(
                            name,
                            Some(&resolved_composite_type.name),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
}
