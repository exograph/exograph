use crate::sql::{column::Column, predicate::Predicate, SQLOperation, Select};

use crate::sql::order::OrderBy;

use anyhow::*;
use payas_model::model::{operation::*, relation::*, types::*};
use payas_model::sql::transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep};
use payas_model::sql::{Limit, Offset};

use super::sql_mapper::{compute_access_predicate, OperationKind};
use super::{
    operation_context::OperationContext,
    sql_mapper::{OperationResolver, SQLMapper},
    Arguments,
};

use async_graphql_parser::{
    types::{Field, Selection, SelectionSet},
    Positioned,
};

use crate::execution::resolver::{GraphQLExecutionError, OutputName};

impl<'a> OperationResolver<'a> for Query {
    fn map_to_sql(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<TransactionScript<'a>> {
        let select = self.operation(&field.node, Predicate::True, operation_context, true)?;
        Ok(TransactionScript::Single(TransactionStep::Concrete(
            ConcreteTransactionStep::new(SQLOperation::Select(select)),
        )))
    }
}

pub trait QueryOperations<'a> {
    fn compute_order_by(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<OrderBy<'a>>;

    fn compute_limit(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Limit>;

    fn compute_offset(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Offset>;

    fn content_select(
        &'a self,
        selection_set: &'a Positioned<SelectionSet>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<&'a Column<'a>>;

    fn operation(
        &'a self,
        field: &'a Field,
        additional_predicate: Predicate<'a>,
        operation_context: &'a OperationContext<'a>,
        top_level_selection: bool,
    ) -> Result<Select<'a>>;
}

impl<'a> QueryOperations<'a> for Query {
    fn compute_order_by(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<OrderBy<'a>> {
        self.order_by_param
            .as_ref()
            .and_then(|order_by_param| {
                let argument_value = super::find_arg(arguments, &order_by_param.name);
                argument_value.map(|argument_value| {
                    order_by_param.map_to_sql(argument_value, operation_context)
                })
            })
            .transpose()
            .unwrap() // TODO: handle properly
    }

    fn content_select(
        &'a self,
        selection_set: &'a Positioned<SelectionSet>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<&'a Column<'a>> {
        let column_specs: Result<Vec<_>> = selection_set
            .node
            .items
            .iter()
            .flat_map(
                |selection| match map_selection(self, &selection.node, operation_context) {
                    Ok(s) => s.into_iter().map(Ok).collect(),
                    Err(err) => vec![Err(err)],
                },
            )
            .collect();

        Ok(operation_context.create_column(Column::JsonObject(column_specs?)))
    }

    fn compute_limit(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Limit> {
        self.limit_param
            .as_ref()
            .and_then(|limit_param| {
                let argument_value = super::find_arg(arguments, &limit_param.name);
                argument_value
                    .map(|argument_value| limit_param.map_to_sql(argument_value, operation_context))
            })
            .transpose()
            .unwrap()
    }

    fn compute_offset(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Offset> {
        self.offset_param
            .as_ref()
            .and_then(|offset_param| {
                let argument_value = super::find_arg(arguments, &offset_param.name);
                argument_value.map(|argument_value| {
                    offset_param.map_to_sql(argument_value, operation_context)
                })
            })
            .transpose()
            .unwrap()
    }

    fn operation(
        &'a self,
        field: &'a Field,
        additional_predicate: Predicate<'a>,
        operation_context: &'a OperationContext<'a>,
        top_level_selection: bool,
    ) -> Result<Select<'a>> {
        let access_predicate = compute_access_predicate(
            &self.return_type,
            &OperationKind::Retrieve,
            operation_context,
        );

        if access_predicate == &Predicate::False {
            bail!(anyhow!(GraphQLExecutionError::Authorization))
        }

        let predicate = super::compute_predicate(
            self.predicate_param.as_ref(),
            &field.arguments,
            additional_predicate,
            operation_context,
        )
        .map(|predicate| {
            operation_context.create_predicate(Predicate::And(
                Box::new(predicate.clone()),
                Box::new(access_predicate.clone()),
            ))
        })
        .with_context(|| format!("While computing predicate for field {}", field.name))?;

        let content_object = self.content_select(&field.selection_set, operation_context)?;

        let table = self
            .return_type
            .physical_table(operation_context.query_context.system);

        let limit = self.compute_limit(&field.arguments, operation_context);
        let offset = self.compute_offset(&field.arguments, operation_context);

        Ok(match self.return_type.type_modifier {
            GqlTypeModifier::Optional | GqlTypeModifier::NonNull => table.select(
                vec![content_object],
                Some(predicate),
                None,
                offset,
                limit,
                top_level_selection,
            ),
            GqlTypeModifier::List => {
                let order_by = self.compute_order_by(&field.arguments, operation_context);
                let agg_column = operation_context.create_column(Column::JsonAgg(content_object));
                table.select(
                    vec![agg_column],
                    Some(predicate),
                    order_by,
                    offset,
                    limit,
                    top_level_selection,
                )
            }
        })
    }
}

fn map_selection<'a>(
    query: &'a Query,
    selection: &'a Selection,
    operation_context: &'a OperationContext<'a>,
) -> Result<Vec<(String, &'a Column<'a>)>> {
    match selection {
        Selection::Field(field) => Ok(vec![map_field(query, &field.node, operation_context)?]),
        Selection::FragmentSpread(fragment_spread) => {
            let fragment_definition = operation_context
                .query_context
                .fragment_definition(fragment_spread)
                .unwrap();
            fragment_definition
                .selection_set
                .node
                .items
                .iter()
                .flat_map(|selection| {
                    match map_selection(query, &selection.node, operation_context) {
                        Ok(s) => s.into_iter().map(Ok).collect(),
                        Err(err) => vec![Err(err)],
                    }
                })
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
    operation_context: &'a OperationContext<'a>,
) -> Result<(String, &'a Column<'a>)> {
    let system = operation_context.query_context.system;
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
                let other_table_pk_query = match other_type.kind {
                    GqlTypeKind::Primitive => panic!(""),
                    GqlTypeKind::Composite(GqlCompositeTypeKind { pk_query, .. }) => {
                        &system.queries[pk_query]
                    }
                };

                Column::SelectionTableWrapper(
                    other_table_pk_query.operation(
                        field,
                        Predicate::Eq(
                            operation_context.create_column_with_id(column_id),
                            operation_context
                                .create_column_with_id(&other_type.pk_column_id().unwrap()),
                        ),
                        operation_context,
                        false,
                    )?,
                )
            }
            GqlRelation::OneToMany {
                other_type_column_id,
                other_type_id,
            } => {
                let other_type = &system.types[*other_type_id];
                let other_table_collection_query = {
                    match other_type.kind {
                        GqlTypeKind::Primitive => panic!(""),
                        GqlTypeKind::Composite(GqlCompositeTypeKind {
                            collection_query, ..
                        }) => &system.queries[collection_query],
                    }
                };

                let other_selection_table = other_table_collection_query.operation(
                    field,
                    Predicate::Eq(
                        operation_context.create_column_with_id(other_type_column_id),
                        operation_context
                            .create_column_with_id(&return_type.pk_column_id().unwrap()),
                    ),
                    operation_context,
                    false,
                )?;

                Column::SelectionTableWrapper(other_selection_table)
            }
        }
    };

    Ok((field.output_name(), operation_context.create_column(column)))
}
