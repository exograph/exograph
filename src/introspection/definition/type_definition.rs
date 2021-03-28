use crate::{
    introspection::util,
    model::types::{ModelField, ModelType, ModelTypeKind::*},
};
use async_graphql_parser::types::{FieldDefinition, ObjectType, TypeDefinition, TypeKind};

use super::provider::{FieldDefinitionProvider, TypeDefinitionProvider};
use crate::introspection::util::*;

impl TypeDefinitionProvider for ModelType {
    fn type_definition(&self) -> TypeDefinition {
        match &self.kind {
            Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            Composite { model_fields, .. } => {
                let fields = model_fields
                    .iter()
                    .map(|model_field| default_positioned(model_field.field_definition()))
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
    fn field_definition(&self) -> FieldDefinition {
        let field_type =
            util::default_positioned(util::value_type(&self.type_name, &self.type_modifier));

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments: vec![],
            ty: field_type,
            directives: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_util::common_test_data::*;
    use async_graphql_parser::parse_schema;

    #[test]
    fn plain() {
        let expected = parse_schema::<&str>(
            "type Venue {
                id: Int!
                name: String
            }",
        )
        .unwrap();

        let system = test_system();
        let venue = system.find_type("Venue").unwrap();
        // assert_eq!(
        //     format!("{}", expected),
        //     format!("{}", venue.type_definition())
        // );
    }

    #[test]
    fn nested() {
        let system = test_system();
        let concert = system.find_type("Concert").unwrap();

        let expected = parse_schema(
            "type Concert {
        id: Int!
        title: String!
        venue: Venue!
      }",
        )
        .unwrap();

        // assert_eq!(
        //     format!("{}", expected),
        //     format!("{}", concert.type_definition())
        // );
    }
}
