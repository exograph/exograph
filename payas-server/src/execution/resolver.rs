use std::{collections::HashSet, iter::FromIterator};

use anyhow::Result;
use async_graphql_parser::{
    types::{Field, Selection, SelectionSet},
    Positioned,
};
use async_trait::async_trait;
use futures::StreamExt;
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
            .unwrap_or_else(|| self.name.node.to_string()))
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

#[async_trait(?Send)]
pub trait Resolver<R> {
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e Positioned<SelectionSet>,
    ) -> Result<R>;
}

#[async_trait(?Send)]
pub trait FieldResolver<R>
where
    Self: std::fmt::Debug,
{
    // {
    //   name: ???
    // }
    // `field` is `name` and ??? is the return value
    async fn resolve_field<'e>(
        &'e self,
        query_context: &'e QueryContext<'e>,
        field: &'e Positioned<Field>,
    ) -> Result<R>;

    // TODO: Move out of the trait to avoid it being overriden?
    async fn resolve_selection(
        &self,
        query_context: &QueryContext<'_>,
        selection: &Positioned<Selection>,
    ) -> Result<Vec<(String, R)>> {
        match &selection.node {
            Selection::Field(field) => Ok(vec![(
                field.output_name(),
                self.resolve_field(query_context, field).await?,
            )]),
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition = query_context.fragment_definition(fragment_spread)?;
                self.resolve_selection_set(query_context, &fragment_definition.selection_set)
                    .await
            }
            Selection::InlineFragment(_inline_fragment) => {
                Ok(vec![]) // TODO
            }
        }
    }

    async fn resolve_selection_set(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
    ) -> Result<Vec<(String, R)>> {
        let selections: Vec<_> = futures::stream::iter(selection_set.node.items.iter())
            .then(|selection| self.resolve_selection(query_context, selection))
            .collect()
            .await;

        selections
            .into_iter()
            .flat_map(|selection| match selection {
                Ok(s) => s.into_iter().map(Ok).collect(),
                Err(err) => vec![Err(err)],
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum GraphQLExecutionError {
    DuplicateKeys(HashSet<String>),
    InvalidField(String, &'static str), // (field name, container type)
    Authorization,
}

impl std::error::Error for GraphQLExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for GraphQLExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphQLExecutionError::DuplicateKeys(duplicates) => {
                // TODO: track lexical positions and sort by those
                let mut keys = duplicates
                    .iter()
                    .map(|u| u.to_string())
                    .collect::<Vec<String>>();
                keys.sort();
                write!(f, "Duplicate keys ({}) in query", keys.join(", "))
            }
            GraphQLExecutionError::InvalidField(field_name, container_name) => {
                write!(f, "Invalid field {} for {}", field_name, container_name)
            }
            GraphQLExecutionError::Authorization => {
                write!(f, "Not authorized")
            }
        }
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

#[async_trait(?Send)]
impl<T> Resolver<Value> for T
where
    T: FieldResolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e Positioned<SelectionSet>,
    ) -> Result<Value> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_selection_set(query_context, selection_set)
                .await?,
        )))
    }
}

#[async_trait(?Send)]
impl<T> Resolver<Value> for Option<&T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e Positioned<SelectionSet>,
    ) -> Result<Value> {
        match self {
            Some(elem) => elem.resolve_value(query_context, selection_set).await,
            None => Ok(Value::Null),
        }
    }
}

#[async_trait(?Send)]
impl<T> Resolver<Value> for Positioned<T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e Positioned<SelectionSet>,
    ) -> Result<Value> {
        self.node.resolve_value(query_context, selection_set).await
    }
}

#[async_trait(?Send)]
impl<T> Resolver<Value> for Vec<T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e Positioned<SelectionSet>,
    ) -> Result<Value> {
        let resolved: Vec<_> = futures::stream::iter(self.iter())
            .then(|elem| elem.resolve_value(query_context, selection_set))
            .collect()
            .await;

        let resolved: Result<Vec<Value>> = resolved.into_iter().collect();

        Ok(Value::Array(resolved?))
    }
}
