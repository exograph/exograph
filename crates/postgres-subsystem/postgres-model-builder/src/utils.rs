use core_plugin_interface::core_model::types::FieldType;
use postgres_model::types::{EntityType, MutationType, PostgresFieldType, TypeIndex};

pub(super) fn to_mutation_type(
    field_type: &FieldType<PostgresFieldType<EntityType>>,
) -> FieldType<PostgresFieldType<MutationType>> {
    match field_type {
        FieldType::Optional(ft) => FieldType::Optional(Box::new(to_mutation_type(ft))),
        FieldType::Plain(PostgresFieldType { type_id, type_name }) => match type_id {
            TypeIndex::Primitive(index) => FieldType::Plain(PostgresFieldType {
                type_id: TypeIndex::Primitive(*index),
                type_name: type_name.clone(),
            }),
            TypeIndex::Composite(_) => panic!(),
        },
        FieldType::List(ft) => FieldType::List(Box::new(to_mutation_type(ft))),
    }
}
