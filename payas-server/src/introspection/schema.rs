use async_graphql_parser::types::{ObjectType, TypeDefinition, TypeKind};

use payas_model::model::system::ModelSystem;

use super::definition::{provider::*, type_introspection::TypeDefinitionIntrospection};
use crate::introspection::util::*;
#[derive(Debug, Clone)]
pub struct Schema {
    pub type_definitions: Vec<TypeDefinition>,
}

impl Schema {
    pub fn new(system: &ModelSystem) -> Schema {
        let mut type_definitions: Vec<TypeDefinition> = system
            .types
            .iter()
            .map(|model_type| model_type.1.type_definition(system))
            .collect();

        let order_by_param_type_definitions: Vec<TypeDefinition> = system
            .order_by_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(system))
            .collect();

        let predicate_param_type_definitions: Vec<TypeDefinition> = system
            .predicate_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(system))
            .collect();

        let mutation_param_type_definitions: Vec<TypeDefinition> = system
            .mutation_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(system))
            .collect();

        let query_type_definition = {
            let fields = system
                .queries
                .values
                .iter()
                .map(|query| default_positioned(query.1.field_definition(system)))
                .collect();

            TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name("Query"),
                directives: vec![],
                kind: TypeKind::Object(ObjectType {
                    implements: vec![],
                    fields,
                }),
            }
        };

        let mutation_type_definition = {
            let fields = system
                .create_mutations
                .values
                .iter()
                .map(|mutation| default_positioned(mutation.1.field_definition(system)))
                .collect();

            TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name("Mutation"),
                directives: vec![],
                kind: TypeKind::Object(ObjectType {
                    implements: vec![],
                    fields,
                }),
            }
        };

        type_definitions.push(query_type_definition);
        type_definitions.push(mutation_type_definition);
        type_definitions.extend(order_by_param_type_definitions);
        type_definitions.extend(predicate_param_type_definitions);
        type_definitions.extend(mutation_param_type_definitions);

        Schema { type_definitions }
    }

    pub fn get_type_definition(&self, type_name: &str) -> Option<&TypeDefinition> {
        self.type_definitions
            .iter()
            .find(|td| td.name().as_str() == type_name)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::common_test_data::test_system;

    use super::*;

    #[test]
    fn schema_generation() {
        let system = test_system();
        let schema = Schema::new(&system);

        schema
            .type_definitions
            .iter()
            .for_each(|td| println!("{}", format!("{:?}", td)));
    }
}
