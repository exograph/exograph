use std::collections::HashSet;

use crate::execution::query_context::QueryContext;
use crate::sql::{column::Column, predicate::Predicate, SQLOperation, Select};

use crate::sql::order::OrderBy;

use anyhow::*;
use maybe_owned::MaybeOwned;
use payas_model::model::system::ModelSystem;
use payas_model::model::{operation::*, relation::*, types::*};
use payas_model::sql::column::PhysicalColumn;
use payas_model::sql::transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep};
use payas_model::sql::{Limit, Offset, PhysicalTable, TableQuery};

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

                let field_arguments = query_context.field_arguments(field)?;

                let predicate = super::compute_predicate(
                    predicate_param.as_ref(),
                    field_arguments,
                    additional_predicate,
                    query_context,
                )
                .map(|predicate| Predicate::and(predicate, access_predicate))
                .with_context(|| format!("While computing predicate for field {}", field.name))?;

                let content_object = self.content_select(&field.selection_set, query_context)?;

                // We may have a predicate that uses one of the referred object. For example, we may have
                // concerts(where: {venue: {name: {eq: "v1"}}}). In such cases, we can't just have `where venue.name = "v1"`
                // since the `from` clause would refer to the `concerts` table, not the `venues` table. So we join the
                // `concerts` table to the `venues` table.
                // To do so, we first find out all the tabled referred by the predicate and then traverse the return type
                // to find out the tables that are referred by the predicate. Along the way, we also pick the join predicate.
                let tables_referred = tables_referred(&predicate, query_context.get_system());
                let TableDependency {
                    table,
                    dependencies,
                } = table_dependency(
                    self.return_type.typ(query_context.get_system()),
                    &tables_referred,
                    query_context,
                );

                // Apply the join logic only for top-level selections (otherwise subselects gets a join and end up selecting
                // a lot more that correct data)
                // TODO: Re-examine a principled way to do this. For example, concerts(where: {veneu: {id: 1}}) { venue {..} } doesn't need
                // any where clause (or join) in the subselect for venue.
                let table = if top_level_selection {
                    dependencies
                        .into_iter()
                        .fold(table, |table, (join_predicate, dependency)| {
                            table.join(dependency.table, join_predicate)
                        })
                } else {
                    table
                };

                let limit = self.compute_limit(field_arguments, query_context);
                let offset = self.compute_offset(field_arguments, query_context);

                Ok(match self.return_type.type_modifier {
                    GqlTypeModifier::Optional | GqlTypeModifier::NonNull => table.select(
                        vec![content_object.into()],
                        predicate,
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
                            predicate,
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
            let fragment_definition = query_context.fragment_definition(fragment_spread)?;
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

/// Table dependencies tree suitable for computing a join.
#[derive(Debug)]
struct TableDependency<'a> {
    table: TableQuery<'a>,
    dependencies: Vec<(MaybeOwned<'a, Predicate<'a>>, TableDependency<'a>)>,
}

/// Compute the dependencies tree for a given root type limiting to the tables referred.
/// Navigate the structure of the root type and pick up any tables referred as long as they are
/// in the `tables_referred` list.
fn table_dependency<'a>(
    root_type: &'a GqlType,
    tables_referred: &[&'a PhysicalTable],
    query_context: &'a QueryContext<'a>,
) -> TableDependency<'a> {
    let system = query_context.get_system();

    if let GqlTypeKind::Composite(composite_root_type) = &root_type.kind {
        let root_physical_table = &system.tables[composite_root_type.get_table_id()];
        let root = TableQuery::Physical(root_physical_table);

        // Drop the current table before descending into its dependencies
        let tables_referred: Vec<&PhysicalTable> = tables_referred
            .iter()
            .filter(|table| table.name != root_physical_table.name)
            .copied()
            .collect();

        let dependencies: Vec<_> = composite_root_type
            .fields
            .iter()
            .filter_map(|field| match &field.relation {
                GqlRelation::ManyToOne {
                    column_id,
                    other_type_id,
                    ..
                }
                | GqlRelation::OneToMany {
                    other_type_column_id: column_id,
                    other_type_id,
                } => {
                    let field_type = field.typ.base_type(&system.types);
                    // TODO: Move the relationship predicate to relation itself (and then use that here and in map_field)
                    let other_type = if matches!(&field.relation, GqlRelation::ManyToOne { .. }) {
                        &system.types[*other_type_id]
                    } else {
                        root_type
                    };

                    if let GqlTypeKind::Composite(composite_root_type) = &field_type.kind {
                        let other_physical_table =
                            &system.tables[composite_root_type.get_table_id()];
                        let join_predicate = Predicate::Eq(
                            query_context.create_column_with_id(column_id).into(),
                            query_context
                                .create_column_with_id(&other_type.pk_column_id().unwrap())
                                .into(),
                        )
                        .into();
                        if tables_referred.contains(&other_physical_table) {
                            Some((
                                join_predicate,
                                table_dependency(field_type, &tables_referred, query_context),
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        TableDependency {
            table: root,
            dependencies,
        }
    } else {
        panic!("Not a composite root type");
    }
}

fn columns_referred<'a>(predicate: &'a Predicate) -> Vec<&'a PhysicalColumn> {
    fn physical_column<'a>(column: &'a Column) -> Option<&'a PhysicalColumn> {
        match column {
            Column::Physical(column) => Some(column),
            _ => None,
        }
    }

    match predicate {
        Predicate::Eq(left, right)
        | Predicate::Neq(left, right)
        | Predicate::Lt(left, right)
        | Predicate::Lte(left, right)
        | Predicate::Gt(left, right)
        | Predicate::Gte(left, right)
        | Predicate::In(left, right)
        | Predicate::StringLike(left, right, _)
        | Predicate::StringStartsWith(left, right)
        | Predicate::StringEndsWith(left, right)
        | Predicate::JsonContains(left, right)
        | Predicate::JsonContainedBy(left, right)
        | Predicate::JsonMatchKey(left, right)
        | Predicate::JsonMatchAnyKey(left, right)
        | Predicate::JsonMatchAllKeys(left, right) => {
            vec![physical_column(left), physical_column(right)]
                .into_iter()
                .flatten()
                .collect()
        }
        Predicate::True | Predicate::False => {
            vec![]
        }
        Predicate::And(left, right) | Predicate::Or(left, right) => columns_referred(left)
            .into_iter()
            .chain(columns_referred(right))
            .collect(),
        Predicate::Not(underlying) => columns_referred(underlying),
    }
}

fn tables_referred<'a>(predicate: &Predicate, system: &'a ModelSystem) -> Vec<&'a PhysicalTable> {
    let mut table_names = HashSet::new();

    for column in columns_referred(predicate) {
        table_names.insert(column.table_name.clone());
    }

    let tables = &system.tables;

    table_names
        .into_iter()
        .map(|table_name| {
            tables
                .iter()
                .find(|t| tables[t.0].name == table_name)
                .unwrap()
                .1
        })
        .collect()
}
