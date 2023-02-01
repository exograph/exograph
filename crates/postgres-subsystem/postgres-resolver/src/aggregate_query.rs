use super::{
    postgres_execution_error::PostgresExecutionError, sql_mapper::SQLOperationKind,
    util::check_access,
};
use crate::operation_resolver::OperationSelectionResolver;
use async_recursion::async_recursion;
use async_trait::async_trait;
use core_plugin_interface::core_resolver::{
    request_context::RequestContext, validation::field::ValidatedField,
};
use futures::StreamExt;
use maybe_owned::MaybeOwned;
use payas_sql::{
    AbstractPredicate, AbstractSelect, ColumnSelection, SelectionCardinality, SelectionElement,
};
use postgres_model::{
    model::ModelPostgresSystem,
    operation::AggregateQuery,
    relation::PostgresRelation,
    types::{EntityType},
};

#[async_trait]
impl OperationSelectionResolver for AggregateQuery {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractSelect<'a>, PostgresExecutionError> {
        let access_predicate = check_access(
            &self.return_type,
            &SQLOperationKind::Retrieve,
            subsystem,
            request_context,
        )
        .await?;

        let parameter = &self.parameter;
        let predicate_param = &parameter.predicate_param;

        let query_predicate = super::predicate_mapper::compute_predicate(
            predicate_param,
            &field.arguments,
            subsystem,
        )?;
        let predicate = AbstractPredicate::and(query_predicate, access_predicate);
        let return_postgres_type = &self.return_type.typ(subsystem);

        let root_physical_table = &subsystem.tables[return_postgres_type.table_id];

        let content_object = content_select(
            return_postgres_type,
            &field.subfields,
            subsystem,
            request_context,
        )
        .await?;

        Ok(AbstractSelect {
            table: root_physical_table,
            selection: payas_sql::Selection::Json(content_object, SelectionCardinality::One),
            predicate,
            order_by: None,
            offset: None,
            limit: None,
        })
    }
}

#[async_recursion]
async fn content_select<'content>(
    return_type: &EntityType,
    fields: &'content [ValidatedField],
    subsystem: &'content ModelPostgresSystem,
    request_context: &'content RequestContext<'content>,
) -> Result<Vec<ColumnSelection<'content>>, PostgresExecutionError> {
    futures::stream::iter(fields.iter())
        .then(|field| async { map_field(return_type, field, subsystem, request_context).await })
        .collect::<Vec<Result<_, _>>>()
        .await
        .into_iter()
        .collect()
}

async fn map_field<'content>(
    return_type: &EntityType,
    field: &'content ValidatedField,
    subsystem: &'content ModelPostgresSystem,
    _request_context: &'content RequestContext<'content>,
) -> Result<ColumnSelection<'content>, PostgresExecutionError> {
    let selection_elem = if field.name == "__typename" {
        SelectionElement::Constant(return_type.name.clone())
    } else {
        let model_field = return_type.field(&field.name).unwrap();

        match &model_field.relation {
            PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
                let column = column_id.get_column(subsystem);
                let elements = field
                    .subfields
                    .iter()
                    .map(|field| {
                        (
                            field.output_name(),
                            MaybeOwned::Owned(SelectionElement::Function {
                                function_name: field.name.to_string(),
                                column,
                            }),
                        )
                    })
                    .collect();
                SelectionElement::Object(elements)
            }
            _ => {
                return Err(PostgresExecutionError::Generic(
                    "Invalid nested aggregation of a composite type".into(),
                ))
            }
        }
    };

    Ok(ColumnSelection::new(field.output_name(), selection_elem))
}
