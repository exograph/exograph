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
    AbstractInsert, AbstractSelect, ColumnId, ColumnValuePair, InsertionElement, InsertionRow,
    ManyToOne, NestedInsertion,
};
use futures::future::{join_all, try_join_all};
use postgres_model::{
    mutation::DataParameter,
    relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation},
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
    pub select: AbstractSelect,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractInsert> for InsertOperation<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractInsert, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.data_param.typ.innermost().type_id];
        let table_id = data_type.table_id;

        let rows = map_argument(data_type, argument, subsystem, request_context).await?;

        let abs_insert = AbstractInsert {
            table_id,
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
    request_context: &'a RequestContext<'a>,
) -> Result<Vec<InsertionRow>, PostgresExecutionError> {
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
    request_context: &'a RequestContext<'a>,
) -> Result<InsertionRow, PostgresExecutionError> {
    let mapped = data_type.fields.iter().map(|field| async move {
        let field_arg = super::util::get_argument_field(argument, &field.name);

        // If the argument has not been supplied, but has a default value, extract it from the context
        let field_arg = match field_arg {
            Some(_) => Ok(field_arg),
            None => {
                if let Some(selection) = &field.dynamic_default_value {
                    subsystem
                        .extract_context_selection(request_context, selection)
                        .await
                } else {
                    Ok(None)
                }
            }
        }
        .ok()?;

        field_arg.map(|field_arg| async move {
            match &field.relation {
                PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
                    map_self_column(*column_id, field, field_arg, subsystem).await
                }

                PostgresRelation::ManyToOne(ManyToOneRelation { relation_id, .. }) => {
                    let ManyToOne { self_column_id, .. } = relation_id.deref(&subsystem.database);
                    map_self_column(self_column_id, field, field_arg, subsystem).await
                }

                PostgresRelation::OneToMany(one_to_many_relation) => {
                    map_foreign(
                        field,
                        field_arg,
                        one_to_many_relation,
                        subsystem,
                        request_context,
                    )
                    .await
                }
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
) -> Result<InsertionElement, PostgresExecutionError> {
    let key_column = key_column_id.get_column(&subsystem.database);
    let argument_value = match &field.relation {
        PostgresRelation::ManyToOne(ManyToOneRelation {
            foreign_pk_field_id,
            ..
        }) => {
            let foreign_type_pk_field_name =
                &foreign_pk_field_id.resolve(&subsystem.entity_types).name;
            match super::util::get_argument_field(argument, foreign_type_pk_field_name) {
                Some(foreign_type_pk_arg) => foreign_type_pk_arg,
                None => {
                    // This can happen if we used a context value for a foreign key
                    // Instead of getting in the `{id: <value>}` format, we get the value directly
                    argument
                }
            }
        }
        _ => argument,
    };

    let value_column = cast::literal_column(argument_value, key_column).with_context(format!(
        "While trying to get literal column for {}.{}",
        subsystem.database.get_table(key_column.table_id).name,
        key_column.name
    ))?;

    Ok(InsertionElement::SelfInsert(ColumnValuePair::new(
        key_column_id,
        value_column,
    )))
}

/// Map foreign elements of a data parameter
/// For example, if the data parameter is `data: {name: "venue-name", concerts: [{<concert-info1>}, {<concert-info2>}]} }
/// this needs to be called for the `concerts` part (which is mapped to a separate table)
async fn map_foreign<'a>(
    field: &'a PostgresField<MutationType>, // "concerts"
    argument: &'a Val,                      // [{<concert-info1>}, {<concert-info2>}]
    one_to_many_relation: &OneToManyRelation,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<InsertionElement, PostgresExecutionError> {
    let field_type = base_type(
        &field.typ,
        &subsystem.primitive_types,
        &subsystem.mutation_types,
    );

    let field_type = match field_type {
        PostgresType::Composite(field_type) => field_type,
        _ => unreachable!("Foreign type cannot be a primitive"), // TODO: Handle this at the type-level
    };

    let insertion = map_argument(field_type, argument, subsystem, request_context).await?;

    Ok(InsertionElement::NestedInsert(NestedInsertion {
        relation_id: one_to_many_relation.relation_id,
        insertions: insertion,
    }))
}
