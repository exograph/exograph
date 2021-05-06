use std::iter::FromIterator;

use async_graphql_parser::{
    types::{Field, Selection, SelectionSet},
    Positioned,
};
use serde_json::Value;

use super::query_context::QueryContext;

pub trait OutputName {
    fn output_name(&self) -> String;
}

impl OutputName for Field {
    fn output_name(&self) -> String {
        (&self
            .alias
            .as_ref()
            .map(|alias| alias.node.to_string())
            .unwrap_or(self.name.node.to_string()))
            .to_string()
    }
}

impl<T> OutputName for Positioned<T>
where
    T: OutputName,
{
    fn output_name(&self) -> String {
        self.node.output_name()
    }
}

pub trait Resolver<R> {
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
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
        field: &Positioned<Field>,
    ) -> R;

    // TODO: Move out of the trait to avoid it being overriden?
    fn resolve_selection(
        &self,
        query_context: &QueryContext<'_>,
        selection: &Positioned<Selection>,
    ) -> Vec<(String, R)> {
        match &selection.node {
            Selection::Field(field) => {
                vec![(
                    field.output_name(),
                    self.resolve_field(query_context, &field),
                )]
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition =
                    query_context.fragment_definition(&fragment_spread).unwrap();
                fragment_definition
                    .selection_set
                    .node
                    .items
                    .iter()
                    .flat_map(|selection| self.resolve_selection(query_context, &selection))
                    .collect()
            }
            Selection::InlineFragment(_inline_fragment) => {
                vec![] // TODO
            }
        }
    }

    fn resolve_selection_set<COL: FromIterator<(String, R)>>(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
    ) -> COL {
        selection_set
            .node
            .items
            .iter()
            .flat_map(|selection| self.resolve_selection(query_context, &selection))
            .collect::<COL>()
    }
}

// This might work after https://github.com/rust-lang/rfcs/blob/master/text/1210-impl-specialization.md
// impl<T> FieldResolver<Value> for Option<&T>
// where
//     T: FieldResolver<Value>,
// {
//     fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field) -> Value {
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
        selection_set: &Positioned<SelectionSet>,
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
        selection_set: &Positioned<SelectionSet>,
    ) -> Value {
        match self {
            Some(elem) => elem.resolve_value(query_context, selection_set),
            None => Value::Null,
        }
    }
}

impl<T> Resolver<Value> for Positioned<T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
    ) -> Value {
        self.node.resolve_value(query_context, selection_set)
    }
}

impl<T> Resolver<Value> for Vec<T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
    ) -> Value {
        let resolved: Vec<Value> = self
            .iter()
            .map(|elem| elem.resolve_value(query_context, selection_set))
            .collect();

        Value::Array(resolved)
    }
}
