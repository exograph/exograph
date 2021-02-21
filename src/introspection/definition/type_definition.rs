use crate::{
    introspection::util,
    model::types::{ModelField, ModelType, ModelTypeKind::*},
};
use graphql_parser::{
    schema::{Field, ObjectType, ScalarType, TypeDefinition},
    Pos,
};

use super::provider::{FieldDefinitionProvider, TypeDefinitionProvider};

impl<'a> TypeDefinitionProvider for ModelType {
    fn type_definition(&self) -> TypeDefinition<String> {
        match &self.kind {
            Primitive => TypeDefinition::Scalar(ScalarType::new(self.name.clone())),
            Composite { model_fields, .. } => {
                let fields = model_fields
                    .iter()
                    .map(|model_field| model_field.field_definition())
                    .collect();

                TypeDefinition::Object(ObjectType {
                    position: Pos::default(),
                    description: None,
                    name: self.name.clone(),
                    implements_interfaces: vec![],
                    directives: vec![],
                    fields: fields,
                })
            }
        }
    }
}

impl<'a> FieldDefinitionProvider<'a> for ModelField {
    fn field_definition(&self) -> Field<'a, String> {
        let field_type = util::value_type(&self.type_name, &self.type_modifier);

        Field::<'a, String> {
            position: Pos::default(),
            description: None,
            name: self.name.clone(),
            arguments: vec![],
            field_type: field_type,
            directives: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_util::common_test_data::*;
    use graphql_parser::schema::parse_schema;

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
        assert_eq!(
            format!("{}", expected),
            format!("{}", venue.type_definition())
        );
    }

    #[test]
    fn nested() {
        let system = test_system();
        let concert = system.find_type("Concert").unwrap();

        let expected = parse_schema::<String>(
            "type Concert {
        id: Int!
        title: String!
        venue: Venue!
      }",
        )
        .unwrap();

        assert_eq!(
            format!("{}", expected),
            format!("{}", concert.type_definition())
        );
    }
}
