use postgres_model::types::{EntityType, FieldType, MutationType, TypeIndex};

pub(super) fn to_mutation_type(field_type: &FieldType<EntityType>) -> FieldType<MutationType> {
    match field_type {
        FieldType::Optional(ft) => FieldType::Optional(Box::new(to_mutation_type(ft))),
        FieldType::Reference { type_id, type_name } => match type_id {
            TypeIndex::Primitive(index) => FieldType::Reference {
                type_id: TypeIndex::Primitive(*index),
                type_name: type_name.clone(),
            },
            TypeIndex::Composite(_) => panic!(),
        },
        FieldType::List(ft) => FieldType::List(Box::new(to_mutation_type(ft))),
    }
}
