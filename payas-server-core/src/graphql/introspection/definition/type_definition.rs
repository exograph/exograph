use crate::graphql::introspection::{definition::provider::InputValueProvider, util};
use async_graphql_parser::types::{
    FieldDefinition, InputObjectType, ObjectType, TypeDefinition, TypeKind,
};
use payas_model::model::{
    operation::{DatabaseQueryParameter, QueryKind},
    relation::GqlRelation,
    system::ModelSystem,
    types::{GqlCompositeType, GqlField, GqlType, GqlTypeKind},
};

use super::provider::{FieldDefinitionProvider, TypeDefinitionProvider};
use crate::graphql::introspection::util::{default_positioned, default_positioned_name};

impl TypeDefinitionProvider for GqlType {
    fn type_definition(&self, system: &ModelSystem) -> TypeDefinition {
        match &self.kind {
            GqlTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            GqlTypeKind::Composite(GqlCompositeType {
                fields: model_fields,
                ..
            }) => {
                let kind = if self.is_input {
                    let fields = model_fields
                        .iter()
                        .map(|model_field| default_positioned(model_field.input_value()))
                        .collect();
                    TypeKind::InputObject(InputObjectType { fields })
                } else {
                    let fields: Vec<_> = model_fields
                        .iter()
                        .map(|model_field| default_positioned(model_field.field_definition(system)))
                        .collect();

                    TypeKind::Object(ObjectType {
                        implements: vec![],
                        fields,
                    })
                };
                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind,
                }
            }
        }
    }
}

impl FieldDefinitionProvider for GqlField {
    fn field_definition(&self, system: &ModelSystem) -> FieldDefinition {
        let field_type = util::default_positioned(util::compute_type(&self.typ));

        let arguments = match self.relation {
            GqlRelation::Pk { .. }
            | GqlRelation::Scalar { .. }
            | GqlRelation::ManyToOne { .. }
            | GqlRelation::NonPersistent => {
                vec![]
            }
            GqlRelation::OneToMany { other_type_id, .. } => {
                let other_type = &system.database_types[other_type_id];
                match &other_type.kind {
                    GqlTypeKind::Primitive => panic!(),
                    GqlTypeKind::Composite(kind) => {
                        let collection_query = kind.get_collection_query();
                        let collection_query = &system.queries[collection_query];

                        match &collection_query.kind {
                            QueryKind::Database(db_query_params) => {
                                let DatabaseQueryParameter {
                                    predicate_param,
                                    order_by_param,
                                    limit_param,
                                    offset_param,
                                } = db_query_params.as_ref();

                                let predicate_parameter_arg =
                                    predicate_param.as_ref().map(|p| p.input_value());
                                let order_by_parameter_arg =
                                    order_by_param.as_ref().map(|p| p.input_value());
                                let limit_arg = limit_param.as_ref().map(|p| p.input_value());
                                let offset_arg = offset_param.as_ref().map(|p| p.input_value());

                                vec![
                                    predicate_parameter_arg,
                                    order_by_parameter_arg,
                                    limit_arg,
                                    offset_arg,
                                ]
                                .into_iter()
                                .flatten()
                                .map(util::default_positioned)
                                .collect()
                            }
                            QueryKind::Service { .. } => panic!(),
                        }
                    }
                }
            }
        };

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments,
            ty: field_type,
            directives: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::model::test_util::common_test_data::*;
    // use async_graphql_parser::parse_schema;

    #[test]
    fn plain() {
        // let expected = parse_schema::<&str>(
        //     "type Venue {
        //         id: Int!
        //         name: String
        //     }",
        // )
        // .unwrap();

        // let system = test_system();
        // let venue = system.find_type("Venue").unwrap();
        // assert_eq!(
        //     format!("{}", expected),
        //     format!("{}", venue.type_definition())
        // );
    }

    #[test]
    fn nested() {
        //     let system = test_system();
        //     let concert = system.find_type("Concert").unwrap();

        //     let expected = parse_schema(
        //         "type Concert {
        //     id: Int!
        //     title: String!
        //     venue: Venue!
        //   }",
        //     )
        //     .unwrap();

        // assert_eq!(
        //     format!("{}", expected),
        //     format!("{}", concert.type_definition())
        // );
    }
}
