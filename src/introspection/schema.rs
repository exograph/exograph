use graphql_parser::{
    schema::{ObjectType, TypeDefinition},
    Pos,
};

use crate::model::system::ModelSystem;

use super::definition::{provider::*, type_introspection::TypeDefinitionIntrospection};

#[derive(Debug, Clone)]
pub struct Schema<'a> {
    pub type_definitions: Vec<TypeDefinition<'a, String>>,
}

impl<'a> Schema<'a> {
    pub fn new(system: &ModelSystem) -> Schema {
        let mut type_definitions: Vec<TypeDefinition<String>> = system
            .types
            .iter()
            .map(|model_type| model_type.type_definition())
            .collect();

        let order_by_param_type_definitions: Vec<TypeDefinition<String>> = system
            .parameter_types
            .order_by_parameter_type_map.values()
            .map(|parameter_type| parameter_type.type_definition())
            .collect();

        let predicate_param_type_definitions: Vec<TypeDefinition<String>> = system
            .parameter_types
            .predicate_parameter_type_map.values()
            .map(|parameter_type| parameter_type.type_definition())
            .collect();

        let query_type_definition = {
            let fields = system
                .queries
                .iter()
                .map(|query| query.field_definition())
                .collect();

            TypeDefinition::Object(ObjectType {
                position: Pos::default(),
                description: None,
                name: "Query".to_string(),
                implements_interfaces: vec![],
                directives: vec![],
                fields: fields,
            })
        };

        type_definitions.push(query_type_definition);
        type_definitions.extend(order_by_param_type_definitions);
        type_definitions.extend(predicate_param_type_definitions);

        Schema {
            type_definitions: type_definitions,
        }
    }

    pub fn get_type_definition(&self, type_name: &str) -> Option<&'a TypeDefinition<'_, String>> {
        self.type_definitions
            .iter()
            .find(|td| td.name().as_str() == type_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_util::common_test_data::*;

    #[test]
    fn schema_generation() {
        let system = test_system();
        let schema = Schema::new(&system);

        schema
            .type_definitions
            .iter()
            .for_each(|td| println!("{}", format!("{}", td)));
    }
}
