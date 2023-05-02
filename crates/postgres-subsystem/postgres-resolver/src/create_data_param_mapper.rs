// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_recursion::async_recursion;
use async_trait::async_trait;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::context::RequestContext;
use core_plugin_interface::core_resolver::context_extractor::ContextExtractor;
use core_plugin_interface::core_resolver::value::Val;
use exo_sql::{
    AbstractInsert, AbstractSelect, ColumnValuePair, InsertionElement, InsertionRow,
    NestedElementRelation, NestedInsertion,
};
use futures::future::{join_all, try_join_all};
use postgres_model::{
    column_id::ColumnId,
    mutation::DataParameter,
    relation::PostgresRelation,
    subsystem::PostgresSubsystem,
    types::{base_type, EntityType, MutationType, PostgresField, PostgresType},
};

use crate::sql_mapper::SQLMapper;

use super::{
    cast,
    postgres_execution_error::{PostgresExecutionError, WithContext},
};

pub struct InsertOperation<'a> {
    pub data_param: &'a DataParameter,
    pub return_type: &'a OperationReturnType<EntityType>,
    pub select: AbstractSelect<'a>,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractInsert<'a>> for InsertOperation<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresSubsystem,
        request_context: &RequestContext<'a>,
    ) -> Result<AbstractInsert<'a>, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.data_param.typ.innermost().type_id];
        let table = data_type.table(subsystem);

        let rows = map_argument(data_type, argument, subsystem, request_context).await?;

        let abs_insert = AbstractInsert {
            table,
            rows,
            selection: self.select,
        };

        Ok(abs_insert)
    }

    fn param_name(&self) -> &str {
        &self.data_param.name
    }
}

pub(crate) async fn map_argument<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    subsystem: &'a PostgresSubsystem,
    request_context: &RequestContext<'a>,
) -> Result<Vec<InsertionRow<'a>>, PostgresExecutionError> {
    match argument {
        Val::List(arguments) => {
            let mapped = arguments
                .iter()
                .map(|argument| map_single(data_type, argument, subsystem, request_context));
            try_join_all(mapped).await
        }
        _ => vec![map_single(data_type, argument, subsystem, request_context).await]
            .into_iter()
            .collect(),
    }
}

/// Map a single item from the data parameter
#[async_recursion]
async fn map_single<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    subsystem: &'a PostgresSubsystem,
    request_context: &RequestContext<'a>,
) -> Result<InsertionRow<'a>, PostgresExecutionError> {
    let mapped = data_type.fields.iter().map(|field| async move {
        // Process fields that map to a column in the current table
        let field_self_column = field.relation.self_column();
        let field_arg = super::util::get_argument_field(argument, &field.name);

        let field_arg = match field_arg {
            Some(field_arg) => Some(field_arg),
            None => {
                if let Some(selection) = &field.dynamic_default_value {
                    // TODO: Revisit once we unified argument types
                    let _default_value = subsystem
                        .extract_context_selection(request_context, selection)
                        .await;
                    None
                } else {
                    None
                }
            }
        };

        field_arg.map(|field_arg| async move {
            match field_self_column {
                Some(field_self_column) => {
                    map_self_column(field_self_column, field, field_arg, subsystem).await
                }
                None => map_foreign(field, field_arg, data_type, subsystem, request_context).await,
            }
        })
    });

    let row = join_all(mapped).await;
    let row = row.into_iter().flatten().collect::<Vec<_>>();
    let row = try_join_all(row).await?;

    Ok(InsertionRow { elems: row })
}

async fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a PostgresField<MutationType>,
    argument: &'a Val,
    subsystem: &'a PostgresSubsystem,
) -> Result<InsertionElement<'a>, PostgresExecutionError> {
    let key_column = key_column_id.get_column(subsystem);
    let argument_value = match &field.relation {
        PostgresRelation::ManyToOne { other_type_id, .. } => {
            // TODO: Include enough information in the ManyToOne relation to not need this much logic here
            let other_type = &subsystem.entity_types[*other_type_id];
            let other_type_pk_field_name = other_type
                .pk_column_id()
                .map(|column_id| &column_id.get_column(subsystem).name)
                .ok_or_else(|| {
                    PostgresExecutionError::Generic(format!(
                        "{} did not have a primary key field when computing many-to-one for {}",
                        other_type.name, field.name
                    ))
                })?;
            match super::util::get_argument_field(argument, other_type_pk_field_name) {
                Some(other_type_pk_arg) => other_type_pk_arg,
                None => todo!(),
            }
        }
        _ => argument,
    };

    let value_column = cast::literal_column(argument_value, key_column).with_context(format!(
        "While trying to get literal column for {}.{}",
        key_column.table_name, key_column.name
    ))?;

    Ok(InsertionElement::SelfInsert(ColumnValuePair::new(
        key_column,
        value_column.into(),
    )))
}

/// Map foreign elements of a data parameter
/// For example, if the data parameter is `data: {name: "venue-name", concerts: [{<concert-info1>}, {<concert-info2>}]} }
/// this needs to be called for the `concerts` part (which is mapped to a separate table)
async fn map_foreign<'a>(
    field: &'a PostgresField<MutationType>,
    argument: &'a Val,
    parent_data_type: &'a MutationType,
    subsystem: &'a PostgresSubsystem,
    request_context: &RequestContext<'a>,
) -> Result<InsertionElement<'a>, PostgresExecutionError> {
    fn underlying_type<'a>(
        data_type: &'a MutationType,
        system: &'a PostgresSubsystem,
    ) -> &'a EntityType {
        &system.entity_types[data_type.entity_type]
    }

    let field_type = base_type(
        &field.typ,
        &subsystem.primitive_types,
        &subsystem.mutation_types,
    );

    let field_type = match field_type {
        PostgresType::Composite(field_type) => field_type,
        _ => todo!(""), // TODO: Handle this at type-level
    };

    // TODO: Cleanup in the next round

    // Find the column corresponding to the primary key in the parent
    // For example, if the mutation is (assume `Venue -> [Concert]` relation)
    // `createVenue(data: {name: "V1", published: true, concerts: [{title: "C1V1", published: true}, {title: "C1V2", published: false}]})`
    // we need to create a column that evaluates to `select "venues"."id" from "venues"`

    let parent_type = underlying_type(parent_data_type, subsystem);
    let parent_table = &subsystem.tables[parent_type.table_id];

    let parent_pk_physical_column = parent_type.pk_column_id().unwrap().get_column(subsystem);

    // Find the column that the current entity refers to in the parent entity
    // In the above example, this would be "venue_id"
    let self_type = underlying_type(field_type, subsystem);
    let self_table = &subsystem.tables[self_type.table_id];
    let self_reference_column = self_type
        .fields
        .iter()
        .find(|self_field| match self_field.relation.self_column() {
            Some(column_id) => match &column_id.get_column(subsystem).typ {
                exo_sql::PhysicalColumnType::ColumnReference {
                    ref_table_name,
                    ref_column_name,
                    ..
                } => {
                    ref_table_name == &parent_pk_physical_column.table_name
                        && ref_column_name == &parent_pk_physical_column.name
                }
                _ => false,
            },
            None => false,
        })
        .unwrap()
        .relation
        .self_column()
        .unwrap()
        .get_column(subsystem);

    let insertion = map_argument(field_type, argument, subsystem, request_context).await?;

    Ok(InsertionElement::NestedInsert(NestedInsertion {
        relation: NestedElementRelation {
            column: self_reference_column,
            table: self_table,
        },
        parent_table,
        insertions: insertion,
    }))
}
