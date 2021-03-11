use std::iter::FromIterator;

use graphql_parser::query::*;
use serde_json::Value;

use super::query_context::QueryContext;

pub trait OutputName<'a> {
    fn output_name(&self) -> String;
}

impl<'a> OutputName<'a> for Field<'a, String> {
    fn output_name(&self) -> String {
        self.alias.clone().unwrap_or(self.name.clone())
    }
}
pub trait Resolver<R> {
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> R;
}

pub trait FieldResolver<R>
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
    ) -> R;

    // TODO: Move out of the trait to avoid it being overriden?
    fn resolve_selection(
        &self,
        query_context: &QueryContext<'_>,
        selection: &Selection<'_, String>,
    ) -> Vec<(String, R)> {
        match selection {
            Selection::Field(field) => {
                vec![(
                    field.output_name(),
                    self.resolve_field(query_context, field),
                )]
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition = query_context.fragment_definition(&fragment_spread).unwrap();
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

    fn resolve_selection_set<COL: FromIterator<(std::string::String, R)>> (
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> COL {
        selection_set
            .items
            .iter()
            .flat_map(|selection| self.resolve_selection(query_context, selection))
            .collect::<COL>()
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


impl<T> Resolver<Value> for T
where
    T: FieldResolver<Value> + std::fmt::Debug,
{
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &SelectionSet<'_, String>,
    ) -> Value {
        Value::Object(self.resolve_selection_set(query_context, selection_set))
    }
}

impl<T> Resolver<Value> for Option<&T>
where
    T: Resolver<Value> + std::fmt::Debug,
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

impl<T> Resolver<Value> for Vec<T>
where
    T: Resolver<Value> + std::fmt::Debug,
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
