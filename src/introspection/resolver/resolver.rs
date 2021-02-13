use graphql_parser::query::*;
use serde_json::{Map, Value};

use crate::introspection::{query_context, util};
use query_context::QueryContext;
use util::*;

pub trait FieldResolver
where
    Self: std::fmt::Debug,
{
    // {
    //   name: ???
    // }
    // `field` is `name` and ??? is the return value
    fn resolve_field<'a>(
        &'a self,
        query_context: &QueryContext<'_>,
        field: &Field<'_, String>,
    ) -> Value;

    // TODO: Move out of the trait to avoid it being overriden?
    fn resolve_selection(
        &self,
        query_context: &QueryContext<'_>,
        selection: &Selection<'_, String>,
    ) -> Vec<(String, Value)> {
        match selection {
            Selection::Field(field) => {
                vec![(
                    field.output_name(),
                    self.resolve_field(query_context, field),
                )]
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition = query_context
                    .fragment_definitions
                    .iter()
                    .find(|fd| fd.name == fragment_spread.fragment_name)
                    .unwrap();
                fragment_definition
                    .selection_set
                    .items
                    .iter()
                    .flat_map(|selection| self.resolve_selection(query_context, selection))
                    .collect()
            }
            Selection::InlineFragment(_inline_fragment) => {
                vec![] // TODO
            }
        }
    }
}

// This might work after https://github.com/rust-lang/rfcs/blob/master/text/1210-impl-specialization.md
// impl<T> FieldResolver for Option<&T>
// where
//     T: FieldResolver,
// {
//     fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field<'_, String>) -> Value {
//         match self {
//             Some(td) => td.resolve_field(query_context, field),
//             None => Value::Null,
//         }
//     }
// }

pub trait Resolver {
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> Value;
}

impl<T> Resolver for T
where
    T: FieldResolver + std::fmt::Debug,
{
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> Value {
        let elems: Map<String, Value> = selection_set
            .items
            .iter()
            .flat_map(|selection| self.resolve_selection(query_context, selection))
            .collect::<Map<String, Value>>();

        Value::Object(elems)
    }
}

impl<T> Resolver for Option<&T>
where
    T: Resolver + std::fmt::Debug,
{
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> Value {
        match self {
            Some(elem) => elem.resolve_value(query_context, selection_set),
            None => Value::Null,
        }
    }
}

impl<T> Resolver for Vec<T>
where
    T: Resolver + std::fmt::Debug,
{
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> Value {
        let resolved: Vec<Value> = self
            .iter()
            .map(|elem| elem.resolve_value(query_context, selection_set))
            .collect();
        Value::Array(resolved)
    }
}
