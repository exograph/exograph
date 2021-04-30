use crate::{
    introspection::{definition::provider::InputValueProvider, util},
    model::{
        relation::ModelRelation,
        system::ModelSystem,
        types::{ModelField, ModelType, *},
    },
};
use async_graphql_parser::types::{FieldDefinition, ObjectType, TypeDefinition, TypeKind};

use super::provider::{FieldDefinitionProvider, TypeDefinitionProvider};
use crate::introspection::util::*;

impl TypeDefinitionProvider for ModelType {
    fn type_definition(&self, system: &ModelSystem) -> TypeDefinition {
        match &self.kind {
            ModelTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            ModelTypeKind::Composite {
                fields: model_fields,
                ..
            } => {
                let fields = model_fields
                    .iter()
                    .map(|model_field| default_positioned(model_field.field_definition(system)))
                    .collect();

                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind: TypeKind::Object(ObjectType {
                        implements: vec![],
                        fields: fields,
                    }),
                }
            }
        }
    }
}

impl FieldDefinitionProvider for ModelField {
    fn field_definition(&self, system: &ModelSystem) -> FieldDefinition {
        let field_type =
            util::default_positioned(util::value_type(&self.type_name, &self.type_modifier));

        let arguments = match self.relation {
            ModelRelation::Pk { .. }
            | ModelRelation::Scalar { .. }
            | ModelRelation::ManyToOne { .. } => vec![],
            ModelRelation::OneToMany { other_type_id, .. } => {
                let other_type = system.types.get_by_id(other_type_id).unwrap();
                match other_type.kind {
                    ModelTypeKind::Primitive => panic!(),
                    ModelTypeKind::Composite {
                        collection_query, ..
                    } => {
                        let collection_query = system.queries.get_by_id(collection_query).unwrap();
                        let predicate_parameter_arg = collection_query
                            .predicate_param
                            .as_ref()
                            .map(|p| p.input_value());
                        let order_by_parameter_arg = collection_query
                            .order_by_param
                            .as_ref()
                            .map(|p| p.input_value());

                        vec![predicate_parameter_arg, order_by_parameter_arg]
                            .into_iter()
                            .flatten()
                            .map(|iv| util::default_positioned(iv))
                            .collect()
                    }
                }
            }
        };

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments: arguments,
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
