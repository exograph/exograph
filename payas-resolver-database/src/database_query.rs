use async_recursion::async_recursion;
use futures::StreamExt;

use payas_model::model::{
    operation::{DatabaseQueryParameter, OperationReturnType, Query, QueryKind},
    relation::{GqlRelation, RelationCardinality},
    types::{GqlTypeKind, GqlTypeModifier},
};
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use payas_sql::{
    AbstractOrderBy, AbstractPredicate, AbstractSelect, ColumnPathLink, ColumnSelection, Limit,
    Offset, SelectionCardinality, SelectionElement,
};

use super::{
    compute_sql_access_predicate,
    database_execution_error::DatabaseExecutionError,
    database_system_context::DatabaseSystemContext,
    order_by_mapper::OrderByParameterMapper,
    sql_mapper::{SQLMapper, SQLOperationKind},
    Arguments,
};

pub struct DatabaseQuery<'a> {
    pub return_type: &'a OperationReturnType,
    pub query_params: &'a DatabaseQueryParameter,
}

impl<'content> DatabaseQuery<'content> {
    pub(super) fn from(query: &Query) -> DatabaseQuery {
        let query_params = match &query.kind {
            QueryKind::Database(query_params) => query_params,
            _ => panic!("DatabaseQuery::from called on non-database query"),
        };

        DatabaseQuery {
            return_type: &query.return_type,
            query_params,
        }
    }

    pub async fn compute_select(
        &self,
        field: &'content ValidatedField,
        additional_predicate: AbstractPredicate<'content>,
        system_context: &DatabaseSystemContext<'content>,
        request_context: &'content RequestContext<'content>,
    ) -> Result<AbstractSelect<'content>, DatabaseExecutionError> {
        let DatabaseQueryParameter {
            predicate_param, ..
        } = self.query_params;
        let access_predicate = compute_sql_access_predicate(
            self.return_type,
            &SQLOperationKind::Retrieve,
            system_context,
            request_context,
        )
        .await;

        if access_predicate == AbstractPredicate::False {
            return Err(DatabaseExecutionError::Authorization);
        }

        let predicate = super::compute_predicate(
            predicate_param.as_ref(),
            &field.arguments,
            additional_predicate,
            system_context,
        )
        .map_err(|e| match e {
            DatabaseExecutionError::Validation(message) => {
                DatabaseExecutionError::Validation(format!(
                    "Error computing predicate for field '{}': {}",
                    field.name, message
                ))
            }
            e => e,
        })?;

        let order_by = self.compute_order_by(&field.arguments, system_context)?;

        let predicate = AbstractPredicate::and(predicate, access_predicate);

        let content_object = self
            .content_select(&field.subfields, system_context, request_context)
            .await?;

        let system = &system_context.system;

        let limit = self.compute_limit(&field.arguments, system_context);
        let offset = self.compute_offset(&field.arguments, system_context);

        let root_physical_table = if let GqlTypeKind::Composite(composite_root_type) =
            &self.return_type.typ(system).kind
        {
            &system.tables[composite_root_type.get_table_id()]
        } else {
            return Err(DatabaseExecutionError::Generic(
                "Expected a composite type".into(),
            ));
        };

        let selection_cardinality = match self.return_type.type_modifier {
            GqlTypeModifier::Optional | GqlTypeModifier::NonNull => SelectionCardinality::One,
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

    fn compute_order_by(
        &self,
        arguments: &'content Arguments,
        system_context: &DatabaseSystemContext<'content>,
    ) -> Result<Option<AbstractOrderBy<'content>>, DatabaseExecutionError> {
        let DatabaseQueryParameter { order_by_param, .. } = self.query_params;
        order_by_param
            .as_ref()
            .and_then(|order_by_param| {
                let argument_value = super::find_arg(arguments, &order_by_param.name);
                argument_value.map(|argument_value| {
                    order_by_param.map_to_order_by(argument_value, None, system_context)
                })
            })
            .transpose()
    }

    #[async_recursion]
    async fn content_select(
        &self,
        fields: &'content [ValidatedField],
        system_context: &DatabaseSystemContext<'content>,
        request_context: &'content RequestContext<'content>,
    ) -> Result<Vec<ColumnSelection<'content>>, DatabaseExecutionError> {
        futures::stream::iter(fields.iter())
            .then(|field| async { self.map_field(field, system_context, request_context).await })
            .collect::<Vec<Result<_, _>>>()
            .await
            .into_iter()
            .collect()
    }

    fn compute_limit(
        &self,
        arguments: &'content Arguments,
        system_context: &DatabaseSystemContext<'content>,
    ) -> Option<Limit> {
        let DatabaseQueryParameter { limit_param, .. } = self.query_params;
        limit_param
            .as_ref()
            .and_then(|limit_param| {
                let argument_value = super::find_arg(arguments, &limit_param.name);
                argument_value
                    .map(|argument_value| limit_param.map_to_sql(argument_value, system_context))
            })
            .transpose()
            .unwrap()
    }

    fn compute_offset(
        &self,
        arguments: &'content Arguments,
        system_context: &DatabaseSystemContext<'content>,
    ) -> Option<Offset> {
        let DatabaseQueryParameter { offset_param, .. } = self.query_params;
        offset_param
            .as_ref()
            .and_then(|offset_param| {
                let argument_value = super::find_arg(arguments, &offset_param.name);
                argument_value
                    .map(|argument_value| offset_param.map_to_sql(argument_value, system_context))
            })
            .transpose()
            .unwrap()
    }

    async fn map_field(
        &self,
        field: &'content ValidatedField,
        system_context: &DatabaseSystemContext<'content>,
        request_context: &'content RequestContext<'content>,
    ) -> Result<ColumnSelection<'content>, DatabaseExecutionError> {
        let system = &system_context.system;
        let return_type = self.return_type.typ(system);

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
                    let other_type = &system.database_types[*other_type_id];
                    let other_table = &system.tables[other_type.table_id().unwrap()];

                    let other_table_pk_query = match &other_type.kind {
                        GqlTypeKind::Primitive => panic!(""),
                        GqlTypeKind::Composite(kind) => {
                            DatabaseQuery::from(&system.queries[kind.get_pk_query()])
                        }
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
                        .compute_select(
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
                    let other_type = &system.database_types[*other_type_id];
                    let other_table_query = {
                        match &other_type.kind {
                            GqlTypeKind::Primitive => panic!(""),
                            GqlTypeKind::Composite(kind) => {
                                // Get an appropriate query based on the cardinality of the relation
                                DatabaseQuery::from(
                                    if cardinality == &RelationCardinality::Unbounded {
                                        &system.queries[kind.get_collection_query()]
                                    } else {
                                        &system.queries[kind.get_pk_query()]
                                    },
                                )
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
                        .compute_select(
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
}
