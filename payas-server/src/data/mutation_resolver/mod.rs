use crate::{
    data::{
        operation_mapper::{compute_sql_access_predicate, SQLOperationKind},
        query_resolver::QuerySQLOperations,
    },
    execution::{query_context::QueryContext, resolver::GraphQLExecutionError},
    sql::{column::Column, Cte, PhysicalTable, SQLOperation},
};

use anyhow::{anyhow, bail, Context, Result};
use payas_model::{
    model::{
        operation::{
            CreateDataParameter, Interceptors, Mutation, MutationKind, Query, UpdateDataParameter,
        },
        predicate::PredicateParameter,
        types::{GqlTypeKind, GqlTypeModifier},
    },
    sql::{
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
        Select,
    },
};
use payas_sql::asql::{
    predicate::AbstractPredicate,
    select::{AbstractSelect, SelectionLevel},
};

use super::operation_mapper::{
    OperationResolver, OperationResolverResult, SQLInsertMapper, SQLUpdateMapper,
};

use async_graphql_parser::{types::Field, Positioned};

impl<'a> OperationResolver<'a> for Mutation {
    fn resolve_operation(
        &'a self,
        field: &'a Positioned<Field>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<OperationResolverResult<'a>> {
        if let MutationKind::Service { method_id, .. } = &self.kind {
            Ok(OperationResolverResult::DenoOperation(method_id.unwrap()))
        } else {
            let abs_select = {
                let (_, pk_query, collection_query) = return_type_info(self, query_context);
                let selection_query = match &self.return_type.type_modifier {
                    GqlTypeModifier::List => collection_query,
                    GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
                };

                selection_query.operation(&field.node, AbstractPredicate::True, query_context)?
            };

            Ok(OperationResolverResult::SQLOperation(match &self.kind {
                MutationKind::Create(data_param) => {
                    create_operation(self, data_param, &field.node, abs_select, query_context)?
                }
                MutationKind::Delete(predicate_param) => {
                    let select = abs_select.to_sql(None, SelectionLevel::TopLevel);
                    delete_operation(self, predicate_param, &field.node, select, query_context)?
                }
                MutationKind::Update {
                    data_param,
                    predicate_param,
                } => update_operation(
                    self,
                    data_param,
                    predicate_param,
                    &field.node,
                    abs_select,
                    query_context,
                )?,
                MutationKind::Service { .. } => panic!(),
            }))
        }
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub fn table_name(mutation: &Mutation, query_context: &QueryContext) -> String {
    mutation
        .return_type
        .physical_table(query_context.get_system())
        .name
        .to_owned()
}

fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a CreateDataParameter,
    field: &'a Field,
    select: AbstractSelect<'a>,
    query_context: &'a QueryContext<'a>,
) -> Result<TransactionScript<'a>> {
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Create,
        query_context,
    );

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let field_arguments = query_context.field_arguments(field)?;
    let argument_value = super::find_arg(field_arguments, &data_param.name).unwrap();

    let abs_insert = data_param.insert_script(mutation, select, argument_value, query_context);

    // let info = insertion_info(data_param, field_arguments, query_context)?.unwrap();

    // let parent_pk_physical_column = info
    //     .table
    //     .get_pk_physical_column()
    //     .expect("Primary key not found");

    // let nested_insert = info
    //     .nested
    //     .into_iter()
    //     .map(|(nested_relation, nested_insertion_info)| {
    //         // let nested_relation = payas_sql::asql::selection::NestedElementRelation {
    //         //     column: parent_pk_physical_column,
    //         //     table: info.table,
    //         // };
    //         NestedAbstractInsert {
    //             relation: nested_relation,
    //             insertion: AbstractInsert {
    //                 table: nested_insertion_info.table,
    //                 column_names: nested_insertion_info.columns,
    //                 column_values_seq: nested_insertion_info.values,
    //                 selection: AbstractSelect {
    //                     table: nested_insertion_info.table,
    //                     selection: Selection::Seq(vec![]),
    //                     predicate: None,
    //                     order_by: None,
    //                     offset: None,
    //                     limit: None,
    //                 },
    //                 nested_insert: None,
    //             },
    //         }
    //     })
    //     .collect();

    // let abs_insert = AbstractInsert {
    //     table: info.table,
    //     column_names: info.columns,
    //     column_values_seq: info.values,
    //     selection: select,
    //     nested_insert: Some(nested_insert),
    // };

    Ok(abs_insert?.to_sql())

    // let ops = info.operation(query_context, true);

    // let mut transaction_script = TransactionScript::default();
    // transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
    //     SQLOperation::Cte(Cte { ctes: ops, select }),
    // )));

    // Ok(transaction_script)
}

fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    select: Select<'a>,
    query_context: &'a QueryContext<'a>,
) -> Result<TransactionScript<'a>> {
    let (table, _, _) = return_type_info(mutation, query_context);

    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Delete,
        query_context,
    );

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        query_context.field_arguments(field)?,
        access_predicate,
        query_context,
    )
    .with_context(|| {
        format!(
            "During predicate computation for parameter {}",
            predicate_param.name
        )
    })?;

    let predicate = predicate.predicate();

    let ops = vec![(
        table_name(mutation, query_context),
        SQLOperation::Delete(table.delete(predicate.into(), vec![Column::Star.into()])),
    )];

    let mut transaction_script = TransactionScript::default();
    transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
        SQLOperation::Cte(Cte { ctes: ops, select }),
    )));

    Ok(transaction_script)
}

fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a UpdateDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    select: AbstractSelect<'a>,
    query_context: &'a QueryContext<'a>,
) -> Result<TransactionScript<'a>> {
    // Access control as well as predicate computation isn't working fully yet. Specifically,
    // nested predicates aren't working.
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Update,
        query_context,
    );

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let field_arguments = query_context.field_arguments(field)?;
    // TODO: https://github.com/payalabs/payas/issues/343
    let predicate = super::compute_predicate(
        Some(predicate_param),
        field_arguments,
        AbstractPredicate::True,
        query_context,
    )
    .with_context(|| {
        format!(
            "During predicate computation for parameter {}",
            predicate_param.name
        )
    })?;

    let argument_value = super::find_arg(field_arguments, &data_param.name);
    argument_value
        .map(|argument_value| {
            data_param
                .update_script(mutation, predicate, select, argument_value, query_context)
                .map(|u| u.to_sql(None))
        })
        .unwrap()
}

// fn insertion_info<'a>(
//     data_param: &'a CreateDataParameter,
//     arguments: &'a Arguments,
//     query_context: &'a QueryContext<'a>,
// ) -> Result<Option<InsertionInfo<'a>>> {
//     let system = &query_context.get_system();
//     let data_type = &system.mutation_types[data_param.type_id];

//     let argument_value = super::find_arg(arguments, &data_param.name);
//     argument_value
//         .map(|argument_value| data_type.map_to_sql(argument_value, query_context))
//         .transpose()
// }

pub fn return_type_info<'a>(
    mutation: &'a Mutation,
    query_context: &'a QueryContext<'a>,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &query_context.get_system();
    let typ = mutation.return_type.typ(system);

    match &typ.kind {
        GqlTypeKind::Primitive => panic!(""),
        GqlTypeKind::Composite(kind) => (
            &system.tables[kind.get_table_id()],
            &system.queries[kind.get_pk_query()],
            &system.queries[kind.get_collection_query()],
        ),
    }
}
