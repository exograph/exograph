use crate::{
    execution::system_context::SystemContext,
    execution_error::{ExecutionError, WithContext},
    request_context::RequestContext,
    validation::field::ValidatedField,
};

use async_trait::async_trait;
use futures::StreamExt;
use payas_model::model::{
    operation::{DatabaseQueryParameter, Interceptors, Query, QueryKind},
    relation::{GqlRelation, RelationCardinality},
    types::{GqlTypeKind, GqlTypeModifier},
};
use payas_sql::{AbstractOperation, AbstractPredicate, ColumnPathLink};
use payas_sql::{AbstractOrderBy, AbstractSelect};
use payas_sql::{ColumnSelection, SelectionCardinality, SelectionElement};
use payas_sql::{Limit, Offset};

use super::{
    operation_mapper::{compute_sql_access_predicate, OperationResolverResult, SQLOperationKind},
    order_by_mapper::OrderByParameterMapper,
};
use super::{
    operation_mapper::{OperationResolver, SQLMapper},
    Arguments,
};

// TODO: deal with panics at the type level

#[async_trait]
impl<'a> OperationResolver<'a> for Query {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError> {
        Ok(match &self.kind {
            QueryKind::Database(_) => {
                let operation = self
                    .operation(
                        field,
                        AbstractPredicate::True,
                        system_context,
                        request_context,
                    )
                    .await?;

                OperationResolverResult::SQLOperation(AbstractOperation::Select(operation))
            }

            QueryKind::Service { method_id, .. } => {
                OperationResolverResult::DenoOperation(method_id.unwrap())
            }
        })
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
pub trait QuerySQLOperations<'a> {
    fn compute_order_by(
        &'a self,
        arguments: &'a Arguments,
        system_context: &'a SystemContext,
    ) -> Option<AbstractOrderBy<'a>>;

    fn compute_limit(
        &'a self,
        arguments: &'a Arguments,
        system_context: &'a SystemContext,
    ) -> Option<Limit>;

    fn compute_offset(
        &'a self,
        arguments: &'a Arguments,
        system_context: &'a SystemContext,
    ) -> Option<Offset>;

    async fn content_select(
        &'a self,
        fields: &'a [ValidatedField],
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Vec<ColumnSelection<'a>>, ExecutionError>;

    async fn operation(
        &'a self,
        field: &'a ValidatedField,
        additional_predicate: AbstractPredicate<'a>,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractSelect<'a>, ExecutionError>;
}

#[async_trait]
impl<'a> QuerySQLOperations<'a> for Query {
    fn compute_order_by(
        &'a self,
        arguments: &'a Arguments,
        system_context: &'a SystemContext,
    ) -> Option<AbstractOrderBy<'a>> {
        match &self.kind {
            QueryKind::Database(db_query_param) => {
                let DatabaseQueryParameter { order_by_param, .. } = db_query_param.as_ref();
                order_by_param
                    .as_ref()
                    .and_then(|order_by_param| {
                        let argument_value = super::find_arg(arguments, &order_by_param.name);
                        argument_value.map(|argument_value| {
                            order_by_param.map_to_order_by(argument_value, &None, system_context)
                        })
                    })
                    .transpose()
                    .unwrap() // TODO: handle properly
            }
            QueryKind::Service { .. } => panic!(),
        }
    }

    async fn content_select(
        &'a self,
        fields: &'a [ValidatedField],
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Vec<ColumnSelection<'a>>, ExecutionError> {
        futures::stream::iter(fields.iter())
            .then(|field| async { map_field(self, field, system_context, request_context).await })
            .collect::<Vec<Result<_, _>>>()
            .await
            .into_iter()
            .collect()
    }

    fn compute_limit(
        &'a self,
        arguments: &'a Arguments,
        system_context: &'a SystemContext,
    ) -> Option<Limit> {
        match &self.kind {
            QueryKind::Database(db_query_param) => {
                let DatabaseQueryParameter { limit_param, .. } = db_query_param.as_ref();
                limit_param
                    .as_ref()
                    .and_then(|limit_param| {
                        let argument_value = super::find_arg(arguments, &limit_param.name);
                        argument_value.map(|argument_value| {
                            limit_param.map_to_sql(argument_value, system_context)
                        })
                    })
                    .transpose()
                    .unwrap()
            }
            QueryKind::Service { .. } => panic!(),
        }
    }

    fn compute_offset(
        &'a self,
        arguments: &'a Arguments,
        system_context: &'a SystemContext,
    ) -> Option<Offset> {
        match &self.kind {
            QueryKind::Database(db_query_param) => {
                let DatabaseQueryParameter { offset_param, .. } = db_query_param.as_ref();
                offset_param
                    .as_ref()
                    .and_then(|offset_param| {
                        let argument_value = super::find_arg(arguments, &offset_param.name);
                        argument_value.map(|argument_value| {
                            offset_param.map_to_sql(argument_value, system_context)
                        })
                    })
                    .transpose()
                    .unwrap()
            }
            QueryKind::Service { .. } => panic!(),
        }
    }

    async fn operation(
        &'a self,
        field: &'a ValidatedField,
        additional_predicate: AbstractPredicate<'a>,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractSelect<'a>, ExecutionError> {
        match &self.kind {
            QueryKind::Database(db_query_param) => {
                let DatabaseQueryParameter {
                    predicate_param, ..
                } = db_query_param.as_ref();
                let access_predicate = compute_sql_access_predicate(
                    &self.return_type,
                    &SQLOperationKind::Retrieve,
                    system_context,
                    request_context,
                )
                .await;

                if access_predicate == AbstractPredicate::False {
                    return Err(ExecutionError::Authorization);
                }

                let predicate = super::compute_predicate(
                    predicate_param.as_ref(),
                    &field.arguments,
                    additional_predicate,
                    system_context,
                )
                .with_context(format!(
                    "While computing predicate for field {}",
                    field.name
                ))?;

                let order_by = self.compute_order_by(&field.arguments, system_context);

                let predicate = AbstractPredicate::and(predicate, access_predicate);

                let content_object = self
                    .content_select(&field.subfields, system_context, request_context)
                    .await?;

                // Apply the join logic only for top-level selections
                let system = &system_context.system;

                let limit = self.compute_limit(&field.arguments, system_context);
                let offset = self.compute_offset(&field.arguments, system_context);

                let root_physical_table = if let GqlTypeKind::Composite(composite_root_type) =
                    &self.return_type.typ(system).kind
                {
                    &system.tables[composite_root_type.get_table_id()]
                } else {
                    return Err(ExecutionError::Generic("Expected a composite type".into()));
                };

                let selection_cardinality = match self.return_type.type_modifier {
                    GqlTypeModifier::Optional | GqlTypeModifier::NonNull => {
                        SelectionCardinality::One
                    }
                    GqlTypeModifier::List => SelectionCardinality::Many,
                };
                let aselect = AbstractSelect {
                    table: root_physical_table,
                    selection: payas_sql::Selection::Json(content_object, selection_cardinality),
                    predicate: Some(predicate),
                    order_by,
                    offset,
                    limit,
                };
                Ok(aselect)
            }

            QueryKind::Service { .. } => {
                todo!()
            }
        }
    }
}

async fn map_field<'a>(
    query: &'a Query,
    field: &'a ValidatedField,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<ColumnSelection<'a>, ExecutionError> {
    let system = &system_context.system;
    let return_type = query.return_type.typ(system);

    let selection_elem = if field.name == "__typename" {
        SelectionElement::Constant(return_type.name.clone())
    } else {
        let model_field = return_type.model_field(&field.name).unwrap();

        match &model_field.relation {
            GqlRelation::Pk { column_id } | GqlRelation::Scalar { column_id } => {
                let column = column_id.get_column(system);
                SelectionElement::Physical(column)
            }
            GqlRelation::ManyToOne {
                column_id,
                other_type_id,
                ..
            } => {
                let other_type = &system.types[*other_type_id];
                let other_table = &system.tables[other_type.table_id().unwrap()];

                let other_table_pk_query = match &other_type.kind {
                    GqlTypeKind::Primitive => panic!(""),
                    GqlTypeKind::Composite(kind) => &system.queries[kind.get_pk_query()],
                };
                let self_table = &system.tables[return_type
                    .table_id()
                    .expect("No table for a composite type")];
                let relation_link = ColumnPathLink {
                    self_column: (column_id.get_column(system), self_table),
                    linked_column: Some((
                        other_table
                            .get_pk_physical_column()
                            .expect("No primary key column found"),
                        other_table,
                    )),
                };

                let nested_abstract_select = other_table_pk_query
                    .operation(
                        field,
                        AbstractPredicate::True,
                        system_context,
                        request_context,
                    )
                    .await?;
                SelectionElement::Nested(relation_link, nested_abstract_select)
            }
            GqlRelation::OneToMany {
                other_type_column_id,
                other_type_id,
                cardinality,
            } => {
                let other_type = &system.types[*other_type_id];
                let other_table_query = {
                    match &other_type.kind {
                        GqlTypeKind::Primitive => panic!(""),
                        GqlTypeKind::Composite(kind) => {
                            // Get an appropriate query based on the cardinality of the relation
                            if cardinality == &RelationCardinality::Unbounded {
                                &system.queries[kind.get_collection_query()]
                            } else {
                                &system.queries[kind.get_pk_query()]
                            }
                        }
                    }
                };
                let self_table = &system.tables[return_type.table_id().unwrap()];
                let self_table_pk_column = self_table
                    .get_pk_physical_column()
                    .expect("No primary key column found");
                let relation_link = ColumnPathLink {
                    self_column: (self_table_pk_column, self_table),
                    linked_column: Some((
                        other_type_column_id.get_column(system),
                        &system.tables[other_type.table_id().unwrap()],
                    )),
                };
                let nested_abstract_select = other_table_query
                    .operation(
                        field,
                        AbstractPredicate::True,
                        system_context,
                        request_context,
                    )
                    .await?;
                SelectionElement::Nested(relation_link, nested_abstract_select)
            }

            _ => {
                panic!("")
            }
        }
    };

    Ok(ColumnSelection::new(field.output_name(), selection_elem))
}
