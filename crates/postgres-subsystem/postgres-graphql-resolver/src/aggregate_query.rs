// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{
    auth_util::{check_access, AccessCheckOutcome},
    sql_mapper::SQLOperationKind,
};

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

use crate::operation_resolver::OperationSelectionResolver;
use async_recursion::async_recursion;
use async_trait::async_trait;
use common::context::RequestContext;
use core_model::types::OperationReturnType;
use core_resolver::validation::field::ValidatedField;
use exo_sql::{
    AbstractPredicate, AbstractSelect, AliasedSelectionElement, SelectionCardinality,
    SelectionElement,
};
use futures::StreamExt;
use std::collections::HashSet;
use postgres_core_model::{relation::PostgresRelation, types::EntityType};
use postgres_graphql_model::{query::AggregateQuery, subsystem::PostgresGraphQLSubsystem};

#[async_trait]
impl OperationSelectionResolver for AggregateQuery {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> Result<AbstractSelect, PostgresExecutionError> {
        let AccessCheckOutcome {
            precheck_predicate: _,
            entity_predicate,
            unauthorized_fields,
        } = check_access(
            self.return_type.typ(&subsystem.core_subsystem.entity_types),
            &field.subfields,
            &SQLOperationKind::Retrieve,
            subsystem,
            request_context,
            None,
        )
        .await?;

        let query_predicate = super::predicate_mapper::compute_predicate(
            &[&self.parameters.predicate_param],
            &field.arguments,
            subsystem,
            request_context,
        )
        .await?;
        let predicate = AbstractPredicate::and(query_predicate, entity_predicate);
        let return_postgres_type = &self.return_type.typ(&subsystem.core_subsystem.entity_types);

        let root_physical_table_id = return_postgres_type.table_id;

        let unauthorized_set = unauthorized_fields.iter().cloned().collect::<HashSet<_>>();

        let content_object = content_select(
            &self.return_type,
            &field.subfields,
            &unauthorized_set,
            subsystem,
            request_context,
        )
        .await?;

        Ok(AbstractSelect {
            table_id: root_physical_table_id,
            selection: exo_sql::Selection::Json(content_object, SelectionCardinality::One),
            predicate,
            order_by: None,
            offset: None,
            limit: None,
        })
    }
}

#[async_recursion]
async fn content_select<'content>(
    return_type: &OperationReturnType<EntityType>,
    fields: &'content [ValidatedField],
    unauthorized_fields: &HashSet<String>,
    subsystem: &'content PostgresGraphQLSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<Vec<AliasedSelectionElement>, PostgresExecutionError> {
    futures::stream::iter(fields.iter())
        .then(|field| async {
            map_field(
                return_type,
                field,
                unauthorized_fields,
                subsystem,
                request_context,
            )
            .await
        })
        .collect::<Vec<Result<_, _>>>()
        .await
        .into_iter()
        .collect()
}

async fn map_field<'content>(
    return_type: &OperationReturnType<EntityType>,
    field: &'content ValidatedField,
    unauthorized_fields: &HashSet<String>,
    subsystem: &'content PostgresGraphQLSubsystem,
    _request_context: &'content RequestContext<'content>,
) -> Result<AliasedSelectionElement, PostgresExecutionError> {
    let output_name = field.output_name();

    let selection_elem = if unauthorized_fields.contains(output_name.as_str()) {
        SelectionElement::Null
    } else if field.name == "__typename" {
        SelectionElement::Constant(return_type.type_name().to_string())
    } else {
        let entity_type = &return_type.typ(&subsystem.core_subsystem.entity_types);

        let model_field = entity_type.field_by_name(&field.name).unwrap();
        let model_field_type = &model_field.typ.innermost().type_name;
        // This is duplicated from builder.
        // We need to rethink aggregation and the concept of aggregate fields in EntityType.
        let model_field_agg_type = format!("{model_field_type}Agg");

        match &model_field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                let elements = field
                    .subfields
                    .iter()
                    .map(|subfield| {
                        let selection_elem = if subfield.name == "__typename" {
                            SelectionElement::Constant(model_field_agg_type.clone())
                        } else {
                            SelectionElement::Function(exo_sql::Function::Named {
                                function_name: subfield.name.to_string(),
                                column_id: *column_id,
                            })
                        };
                        (subfield.output_name(), selection_elem)
                    })
                    .collect();
                SelectionElement::Object(elements)
            }

            _ => {
                return Err(PostgresExecutionError::Generic(
                    "Invalid nested aggregation of a composite type".into(),
                ));
            }
        }
    };

    Ok(AliasedSelectionElement::new(
        output_name,
        selection_elem,
    ))
}
