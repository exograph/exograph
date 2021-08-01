use std::{collections::HashSet, iter::FromIterator};

use anyhow::Result;
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

pub trait Resolver<R> {
    fn resolve_value(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
    ) -> Result<R>;
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
    ) -> Result<R>;

    // TODO: Move out of the trait to avoid it being overriden?
    fn resolve_selection(
        &self,
        query_context: &QueryContext<'_>,
        selection: &Positioned<Selection>,
    ) -> Result<Vec<(String, R)>> {
        match &selection.node {
            Selection::Field(field) => Ok(vec![(
                field.output_name(),
                self.resolve_field(query_context, field)?,
            )]),
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition =
                    query_context.fragment_definition(fragment_spread).unwrap();
                self.resolve_selection_set(query_context, &fragment_definition.selection_set)
            }
            Selection::InlineFragment(_inline_fragment) => {
                Ok(vec![]) // TODO
            }
        }
    }

    fn resolve_selection_set(
        &self,
        query_context: &QueryContext<'_>,
        selection_set: &Positioned<SelectionSet>,
    ) -> Result<Vec<(String, R)>> {
        let resolved: Result<Vec<(String, R)>> = selection_set
            .node
            .items
            .iter()
            .flat_map(
                |selection| match self.resolve_selection(query_context, selection) {
                    Ok(s) => s.into_iter().map(Ok).collect(),
                    Err(err) => vec![Err(err)],
                },
            )
            .collect();
        let resolved = resolved?;

        check_duplicate_keys(&resolved)?;

        Ok(resolved)
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

pub fn check_duplicate_keys<R>(resolved: &[(String, R)]) -> Result<(), GraphQLExecutionError> {
    let mut names = HashSet::new();
    let mut duplicates = HashSet::new();

    resolved.iter().for_each(|(name, _)| {
        if names.contains(name) {
            duplicates.insert(name.to_owned());
        } else {
            names.insert(name);
        }
    });

    if duplicates.is_empty() {
        Ok(())
    } else {
        Err(GraphQLExecutionError::DuplicateKeys(duplicates))
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
    ) -> Result<Value> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_selection_set(query_context, selection_set)?,
        )))
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
    ) -> Result<Value> {
        match self {
            Some(elem) => elem.resolve_value(query_context, selection_set),
            None => Ok(Value::Null),
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
    ) -> Result<Value> {
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
    ) -> Result<Value> {
        let resolved: Result<Vec<Value>> = self
            .iter()
            .map(|elem| elem.resolve_value(query_context, selection_set))
            .collect();

        Ok(Value::Array(resolved?))
    }
}
