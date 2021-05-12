use async_graphql_parser::{
    types::{BaseType, Type},
    Pos, Positioned,
};
use async_graphql_value::Name;

use payas_model::model::types::{ModelTypeModifier, ModelTypeModifier::*};

pub fn default_positioned<T>(value: T) -> Positioned<T> {
    Positioned::new(value, Pos::default())
}

pub fn default_positioned_name(value: &str) -> Positioned<Name> {
    default_positioned(Name::new(value))
}

pub fn value_type(name: &str, type_modifier: &ModelTypeModifier) -> Type {
    let base_field_type = BaseType::Named(Name::new(name));
    match type_modifier {
        Optional => Type {
            base: base_field_type,
            nullable: true,
        },
        NonNull => Type {
            base: base_field_type,
            nullable: false,
        },
        List => Type {
            base: BaseType::List(Box::new(Type {
                base: base_field_type,
                nullable: true,
            })),
            nullable: true,
        },
    }
}
