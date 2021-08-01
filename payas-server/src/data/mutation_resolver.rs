use std::collections::HashSet;

use crate::{
    data::{
        query_resolver::QueryOperations,
        sql_mapper::{compute_access_predicate, OperationKind},
    },
    execution::resolver::GraphQLExecutionError,
    sql::{column::Column, predicate::Predicate, Cte, PhysicalTable, SQLOperation},
};

use anyhow::{anyhow, bail, Result};
use payas_model::{
    model::{operation::*, predicate::PredicateParameter, types::*},
    sql::column::PhysicalColumn,
};

use super::{
    operation_context::OperationContext,
    sql_mapper::{OperationResolver, SQLMapper},
};

use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::{Name, Value};

type Arguments = [(Positioned<Name>, Positioned<Value>)];

impl<'a> OperationResolver<'a> for Mutation {
    fn map_to_sql(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<SQLOperation<'a>> {
        let core_operation = match &self.kind {
            MutationKind::Create(data_param) => {
                create_operation(self, data_param, &field.node, operation_context)
            }
            MutationKind::Delete(predicate_param) => {
                delete_operation(self, predicate_param, &field.node, operation_context)
            }
            MutationKind::Update {
                data_param,
                predicate_param,
            } => update_operation(
                self,
                data_param,
                predicate_param,
                &field.node,
                operation_context,
            )?,
        };

        let (_, pk_query, collection_query) = return_type_info(self, operation_context);
        let selection_query = match &self.return_type.type_modifier {
            GqlTypeModifier::List => collection_query,
            GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
        };

        let select =
            selection_query.operation(&field.node, Predicate::True, operation_context, true)?;

        // Use the same name as the table in the select clause, since that is the name `pk_query.operation` uses
        let cte_name = format!("\"{}\"", select.underlying.name);

        Ok(SQLOperation::Cte(Cte {
            ctes: vec![(cte_name, core_operation)],
            select,
        }))
    }
}

fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a MutationDataParameter,
    field: &'a Field,
    operation_context: &'a OperationContext<'a>,
) -> SQLOperation<'a> {
    let access_predicate = compute_access_predicate(
        &mutation.return_type,
        &OperationKind::Create,
        operation_context,
    );

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate != &Predicate::True {
        panic!("Insufficient access to create"); // TODO: Report as GraphQL error
    }

    let (table, _, _) = return_type_info(mutation, operation_context);

    let (column_names, column_values_seq) =
        insertion_columns(data_param, &field.arguments, operation_context).unwrap();

    SQLOperation::Insert(table.insert(
        column_names,
        column_values_seq,
        vec![operation_context.create_column(Column::Star)],
    ))
}

fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    operation_context: &'a OperationContext<'a>,
) -> SQLOperation<'a> {
    let (table, _, _) = return_type_info(mutation, operation_context);

    let access_predicate = compute_access_predicate(
        &mutation.return_type,
        &OperationKind::Delete,
        operation_context,
    );

    if access_predicate == &Predicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        panic!("Insufficient access to delete"); // TODO: Report as GraphQL error
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        access_predicate.clone(),
        operation_context,
    );

    SQLOperation::Delete(table.delete(
        predicate,
        vec![operation_context.create_column(Column::Star)],
    ))
}

fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a MutationDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    operation_context: &'a OperationContext<'a>,
) -> Result<SQLOperation<'a>> {
    let (table, _, _) = return_type_info(mutation, operation_context);

    let access_predicate = compute_access_predicate(
        &mutation.return_type,
        &OperationKind::Update,
        operation_context,
    );

    if access_predicate == &Predicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        Predicate::True,
        operation_context,
    )
    .unwrap();

    let column_values = data_columns(data_param, &field.arguments, operation_context).unwrap();

    Ok(SQLOperation::Update(table.update(
        column_values,
        predicate,
        vec![operation_context.create_column(Column::Star)],
    )))
}

fn insertion_columns<'a>(
    data_param: &'a MutationDataParameter,
    arguments: &'a Arguments,
    operation_context: &'a OperationContext<'a>,
) -> Option<(Vec<&'a PhysicalColumn>, Vec<Vec<&'a Column<'a>>>)> {
    let argument_value = super::find_arg(arguments, &data_param.name);
    argument_value.map(|argument_value| match argument_value {
        Value::List(elems) => {
            let unaligned: Vec<_> = elems
                .iter()
                .map(|elem| data_param.map_to_sql(elem, operation_context))
                .collect();

            // Here we may have each mapped element with potentially different set of columns.
            // For example, if the input is {data: [{a: 1, b: 2}, {a: 3, c: 4}]}, we will have the 'a' key in both
            // but only 'b' or 'c' keys in others. So we need align columns that can be supplied to an insert statement
            // (a, b, c), [(1, 2, null), (3, null, 4)]
            let mut all_keys = HashSet::new();
            for item in unaligned.iter() {
                for (key, _) in item.iter() {
                    all_keys.insert(*key);
                }
            }

            let keys_count = all_keys.len();

            let mut result = Vec::with_capacity(unaligned.len());
            for item in unaligned.into_iter() {
                let mut row = Vec::with_capacity(keys_count);
                for key in &all_keys {
                    // TODO: We can probably do a better job than find(), maybe preconvert each row to a HashMap, but that too
                    // will require an O(N) operations. Perhaps, data_param.map_to_sql() itself should build and return a HashMap
                    let value = item
                        .iter()
                        .find(|entry| &entry.0 == key)
                        .map(|e| e.1)
                        .unwrap_or(&Column::Null);
                    row.push(value);
                }

                result.push(row);
            }

            (all_keys.into_iter().collect(), result)
        }
        _ => {
            let raw: (Vec<_>, Vec<_>) = data_param
                .map_to_sql(argument_value, operation_context)
                .into_iter()
                .unzip();
            (raw.0, vec![raw.1])
        }
    })
}

fn data_columns<'a>(
    data_param: &'a MutationDataParameter,
    arguments: &'a Arguments,
    operation_context: &'a OperationContext<'a>,
) -> Option<Vec<(&'a PhysicalColumn, &'a Column<'a>)>> {
    let argument_value = super::find_arg(arguments, &data_param.name);
    argument_value.map(|argument_value| data_param.map_to_sql(argument_value, operation_context))
}

fn return_type_info<'a>(
    mutation: &'a Mutation,
    operation_context: &'a OperationContext<'a>,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &operation_context.query_context.system;
    let typ = mutation.return_type.typ(system);

    match typ.kind {
        GqlTypeKind::Primitive => panic!(""),
        GqlTypeKind::Composite(GqlCompositeTypeKind {
            table_id,
            pk_query,
            collection_query,
            ..
        }) => (
            &system.tables[table_id],
            &system.queries[pk_query],
            &system.queries[collection_query],
        ),
    }
}
