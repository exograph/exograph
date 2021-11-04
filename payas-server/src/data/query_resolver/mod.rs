use crate::execution::query_context::QueryContext;
use crate::sql::{column::Column, predicate::Predicate, SQLOperation, Select};

use crate::sql::order::OrderBy;

use anyhow::*;
use maybe_owned::MaybeOwned;
use payas_model::model::{operation::*, relation::*, types::*};
use payas_model::sql::transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep};
use payas_model::sql::{Limit, Offset};

use super::operation_mapper::{
    compute_sql_access_predicate, OperationResolverResult, SQLOperationKind,
};
use super::{
    operation_mapper::{OperationResolver, SQLMapper},
    Arguments,
};

use async_graphql_parser::{
    types::{Field, Selection, SelectionSet},
    Positioned,
};

use crate::execution::resolver::{GraphQLExecutionError, OutputName};

// TODO: deal with panics at the type level

impl<'a> OperationResolver<'a> for Query {
    fn resolve_operation(
        &'a self,
        field: &'a Positioned<Field>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<OperationResolverResult<'a>> {
        match &self.kind {
            QueryKind::Database(_) => {
                let select = self.operation(&field.node, Predicate::True, query_context, true)?;
                Ok(OperationResolverResult::SQLOperation(
                    TransactionScript::Single(TransactionStep::Concrete(
                        ConcreteTransactionStep::new(SQLOperation::Select(select)),
                    )),
                ))
            }

            QueryKind::Service { method_id, .. } => {
                Ok(OperationResolverResult::DenoOperation(method_id.unwrap()))
            }
        }
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub trait QuerySQLOperations<'a> {
    fn compute_order_by(
        &'a self,
        arguments: &'a Arguments,
        query_context: &'a QueryContext<'a>,
    ) -> Option<OrderBy<'a>>;

    fn compute_limit(
        &'a self,
        arguments: &'a Arguments,
        query_context: &'a QueryContext<'a>,
    ) -> Option<Limit>;

    fn compute_offset(
        &'a self,
        arguments: &'a Arguments,
        query_context: &'a QueryContext<'a>,
    ) -> Option<Offset>;

    fn content_select(
        &'a self,
        selection_set: &'a Positioned<SelectionSet>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<Column<'a>>;

    fn operation(
        &'a self,
        field: &'a Field,
        additional_predicate: Predicate<'a>,
        query_context: &'a QueryContext<'a>,
        top_level_selection: bool,
    ) -> Result<Select<'a>>;
}

impl<'a> QuerySQLOperations<'a> for Query {
    fn compute_order_by(
        &'a self,
        arguments: &'a Arguments,
        query_context: &'a QueryContext<'a>,
    ) -> Option<OrderBy<'a>> {
        match &self.kind {
            QueryKind::Database(DatabaseQueryParameter { order_by_param, .. }) => {
                order_by_param
                    .as_ref()
                    .and_then(|order_by_param| {
                        let argument_value = super::find_arg(arguments, &order_by_param.name);
                        argument_value.map(|argument_value| {
                            order_by_param.map_to_sql(argument_value, query_context)
                        })
                    })
                    .transpose()
                    .unwrap() // TODO: handle properly
            }
            QueryKind::Service { .. } => panic!(),
        }
    }

    fn content_select(
        &'a self,
        selection_set: &'a Positioned<SelectionSet>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<Column<'a>> {
        let column_specs: Result<Vec<_>> = selection_set
            .node
            .items
            .iter()
            .flat_map(
                |selection| match map_selection(self, &selection.node, query_context) {
                    Ok(s) => s.into_iter().map(Ok).collect(),
                    Err(err) => vec![Err(err)],
                },
            )
            .collect();

        Ok(Column::JsonObject(column_specs?))
    }

    fn compute_limit(
        &'a self,
        arguments: &'a Arguments,
        query_context: &'a QueryContext<'a>,
    ) -> Option<Limit> {
        match &self.kind {
            QueryKind::Database(DatabaseQueryParameter { limit_param, .. }) => limit_param
                .as_ref()
                .and_then(|limit_param| {
                    let argument_value = super::find_arg(arguments, &limit_param.name);
                    argument_value
                        .map(|argument_value| limit_param.map_to_sql(argument_value, query_context))
                })
                .transpose()
                .unwrap(),
            QueryKind::Service { .. } => panic!(),
        }
    }

    fn compute_offset(
        &'a self,
        arguments: &'a Arguments,
        query_context: &'a QueryContext<'a>,
    ) -> Option<Offset> {
        match &self.kind {
            QueryKind::Database(DatabaseQueryParameter { offset_param, .. }) => offset_param
                .as_ref()
                .and_then(|offset_param| {
                    let argument_value = super::find_arg(arguments, &offset_param.name);
                    argument_value.map(|argument_value| {
                        offset_param.map_to_sql(argument_value, query_context)
                    })
                })
                .transpose()
                .unwrap(),
            QueryKind::Service { .. } => panic!(),
        }
    }

    fn operation(
        &'a self,
        field: &'a Field,
        additional_predicate: Predicate<'a>,
        query_context: &'a QueryContext<'a>,
        top_level_selection: bool,
    ) -> Result<Select<'a>> {
        match &self.kind {
            QueryKind::Database(DatabaseQueryParameter {
                predicate_param, ..
            }) => {
                let access_predicate = compute_sql_access_predicate(
                    &self.return_type,
                    &SQLOperationKind::Retrieve,
                    query_context,
                );

                if access_predicate == Predicate::False {
                    bail!(anyhow!(GraphQLExecutionError::Authorization))
                }

                let field_arguments = query_context.field_arguments(field);

                let predicate = super::compute_predicate(
                    predicate_param.as_ref(),
                    field_arguments,
                    additional_predicate.into(),
                    query_context,
                )
                .map(|predicate| {
                    Predicate::And(Box::new(predicate), Box::new(access_predicate.into()))
                })
                .with_context(|| format!("While computing predicate for field {}", field.name))?;

                let content_object = self.content_select(&field.selection_set, query_context)?;

                let table = self.return_type.physical_table(query_context.get_system());

                let field_arguments = query_context.field_arguments(field);
                let limit = self.compute_limit(field_arguments, query_context);
                let offset = self.compute_offset(field_arguments, query_context);

                Ok(match self.return_type.type_modifier {
                    GqlTypeModifier::Optional | GqlTypeModifier::NonNull => table.select(
                        vec![content_object.into()],
                        Some(predicate),
                        None,
                        offset,
                        limit,
                        top_level_selection,
                    ),
                    GqlTypeModifier::List => {
                        let order_by = self.compute_order_by(field_arguments, query_context);
                        let agg_column = Column::JsonAgg(Box::new(content_object.into()));
                        table.select(
                            vec![agg_column.into()],
                            Some(predicate),
                            order_by,
                            offset,
                            limit,
                            top_level_selection,
                        )
                    }
                })
            }

            QueryKind::Service { .. } => {
                todo!()
            }
        }
    }
}

fn map_selection<'a>(
    query: &'a Query,
    selection: &'a Selection,
    query_context: &'a QueryContext<'a>,
) -> Result<Vec<(String, MaybeOwned<'a, Column<'a>>)>> {
    match selection {
        Selection::Field(field) => Ok(vec![map_field(query, &field.node, query_context)?]),
        Selection::FragmentSpread(fragment_spread) => {
            let fragment_definition = query_context.fragment_definition(fragment_spread).unwrap();
            fragment_definition
                .selection_set
                .node
                .items
                .iter()
                .flat_map(
                    |selection| match map_selection(query, &selection.node, query_context) {
                        Ok(s) => s.into_iter().map(Ok).collect(),
                        Err(err) => vec![Err(err)],
                    },
                )
                .collect()
        }
        Selection::InlineFragment(_inline_fragment) => {
            Ok(vec![]) // TODO
        }
    }
}

fn map_field<'a>(
    query: &'a Query,
    field: &'a Field,
    query_context: &'a QueryContext<'a>,
) -> Result<(String, MaybeOwned<'a, Column<'a>>)> {
    let system = query_context.get_system();
    let return_type = query.return_type.typ(system);

    let column = if field.name.node == "__typename" {
        Column::Constant(return_type.name.clone())
    } else {
        let model_field = return_type.model_field(&field.name.node).unwrap();

        match &model_field.relation {
            GqlRelation::Pk { column_id } | GqlRelation::Scalar { column_id } => {
                let column = column_id.get_column(system);
                Column::Physical(column)
            }
            GqlRelation::ManyToOne {
                column_id,
                other_type_id,
                ..
            } => {
                let other_type = &system.types[*other_type_id];
                let other_table_pk_query = match &other_type.kind {
                    GqlTypeKind::Primitive => panic!(""),
                    GqlTypeKind::Composite(kind) => &system.queries[kind.get_pk_query()],
                };

                Column::SelectionTableWrapper(Box::new(
                    other_table_pk_query.operation(
                        field,
                        Predicate::Eq(
                            query_context.create_column_with_id(column_id).into(),
                            query_context
                                .create_column_with_id(&other_type.pk_column_id().unwrap())
                                .into(),
                        ),
                        query_context,
                        false,
                    )?,
                ))
            }
            GqlRelation::OneToMany {
                other_type_column_id,
                other_type_id,
            } => {
                let other_type = &system.types[*other_type_id];
                let other_table_collection_query = {
                    match &other_type.kind {
                        GqlTypeKind::Primitive => panic!(""),
                        GqlTypeKind::Composite(kind) => {
                            &system.queries[kind.get_collection_query()]
                        }
                    }
                };

                let other_selection_table = other_table_collection_query.operation(
                    field,
                    Predicate::Eq(
                        query_context
                            .create_column_with_id(other_type_column_id)
                            .into(),
                        query_context
                            .create_column_with_id(&return_type.pk_column_id().unwrap())
                            .into(),
                    ),
                    query_context,
                    false,
                )?;

                Column::SelectionTableWrapper(Box::new(other_selection_table))
            }
            GqlRelation::NonPersistent => panic!(),
        }
    };

    Ok((field.output_name(), column.into()))
}
