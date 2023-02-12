use core_plugin_interface::core_model::types::DecoratedType;
use postgres_model::types::{EntityType, MutationType, PostgresFieldType, TypeIndex};

pub(super) fn to_mutation_type(
    field_type: &DecoratedType<PostgresFieldType<EntityType>>,
) -> DecoratedType<PostgresFieldType<MutationType>> {
    match field_type {
        DecoratedType::Optional(ft) => DecoratedType::Optional(Box::new(to_mutation_type(ft))),
        DecoratedType::Plain(PostgresFieldType { type_id, type_name }) => match type_id {
            TypeIndex::Primitive(index) => DecoratedType::Plain(PostgresFieldType {
                type_id: TypeIndex::Primitive(*index),
                type_name: type_name.clone(),
            }),
            TypeIndex::Composite(_) => panic!(),
        },
        DecoratedType::List(ft) => DecoratedType::List(Box::new(to_mutation_type(ft))),
    }
}
