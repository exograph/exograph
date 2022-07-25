use async_graphql_parser::{
    types::{BaseType, Type},
    Pos, Positioned,
};
use async_graphql_value::Name;

use payas_model::model::{
    types::{GqlTypeModifier, GqlTypeModifier::*},
    GqlFieldType,
};

pub fn default_positioned<T>(value: T) -> Positioned<T> {
    Positioned::new(value, Pos::default())
}

pub fn default_positioned_name(value: &str) -> Positioned<Name> {
    default_positioned(Name::new(value))
}

pub fn compute_type(typ: &GqlFieldType) -> Type {
    fn compute_base_type(typ: &GqlFieldType) -> BaseType {
        match typ {
            GqlFieldType::Optional(underlying) => compute_base_type(underlying),
            GqlFieldType::Reference { type_name, .. } => BaseType::Named(Name::new(type_name)),
            GqlFieldType::List(underlying) => BaseType::List(Box::new(compute_type(underlying))),
        }
    }

    match typ {
        GqlFieldType::Optional(underlying) => Type {
            base: compute_base_type(underlying),
            nullable: true,
        },
        GqlFieldType::Reference { type_name, .. } => Type {
            base: BaseType::Named(Name::new(type_name)),
            nullable: false,
        },
        GqlFieldType::List(underlying) => Type {
            base: BaseType::List(Box::new(compute_type(underlying))),
            nullable: false,
        },
    }
}

pub fn value_type(name: &str, type_modifier: &GqlTypeModifier) -> Type {
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
