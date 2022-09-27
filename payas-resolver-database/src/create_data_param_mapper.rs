use async_graphql_value::ConstValue;

use payas_database_model::{
    column_id::ColumnId,
    model::ModelDatabaseSystem,
    operation::{CreateDataParameter, OperationReturnType},
    relation::DatabaseRelation,
    types::{DatabaseCompositeType, DatabaseField, DatabaseType, DatabaseTypeKind},
};
use payas_sql::{
    AbstractInsert, AbstractSelect, ColumnValuePair, InsertionElement, InsertionRow,
    NestedElementRelation, NestedInsertion,
};

use super::{
    cast,
    database_execution_error::{DatabaseExecutionError, WithContext},
    database_system_context::DatabaseSystemContext,
    sql_mapper::SQLInsertMapper,
};

impl<'a> SQLInsertMapper<'a> for CreateDataParameter {
    fn insert_operation(
        &'a self,
        return_type: OperationReturnType,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        system_context: &DatabaseSystemContext<'a>,
    ) -> Result<AbstractInsert, DatabaseExecutionError> {
        let system = &system_context.system;

        let table = return_type.physical_table(system);

        let data_type = &system.mutation_types[self.typ.type_id];

        let rows = map_argument(data_type, argument, system_context)?;

        let abs_insert = AbstractInsert {
            table,
            rows,
            selection: select,
        };

        Ok(abs_insert)
    }
}

pub(crate) fn map_argument<'a>(
    input_data_type: &'a DatabaseType,
    argument: &'a ConstValue,
    system_context: &DatabaseSystemContext<'a>,
) -> Result<Vec<InsertionRow<'a>>, DatabaseExecutionError> {
    match argument {
        ConstValue::List(arguments) => arguments
            .iter()
            .map(|argument| map_single(input_data_type, argument, system_context))
            .collect::<Result<Vec<_>, _>>(),
        _ => vec![map_single(input_data_type, argument, system_context)]
            .into_iter()
            .collect(),
    }
}

/// Map a single item from the data parameter
fn map_single<'a>(
    input_data_type: &'a DatabaseType,
    argument: &'a ConstValue,
    system_context: &DatabaseSystemContext<'a>,
) -> Result<InsertionRow<'a>, DatabaseExecutionError> {
    let fields = match &input_data_type.kind {
        DatabaseTypeKind::Primitive => {
            return Err(DatabaseExecutionError::Generic(
                "Query attempted on a primitive type".into(),
            ))
        }
        DatabaseTypeKind::Composite(DatabaseCompositeType { fields, .. }) => fields,
    };

    let row: Result<Vec<_>, _> = fields
        .iter()
        .flat_map(|field| {
            // Process fields that map to a column in the current table
            let field_self_column = field.relation.self_column();
            let field_arg = super::get_argument_field(argument, &field.name);

            field_arg.map(|field_arg| match field_self_column {
                Some(field_self_column) => {
                    map_self_column(field_self_column, field, field_arg, system_context)
                }
                None => map_foreign(field, field_arg, input_data_type, system_context),
            })
        })
        .collect();

    Ok(InsertionRow { elems: row? })
}

fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a DatabaseField,
    argument: &'a ConstValue,
    system_context: &DatabaseSystemContext<'a>,
) -> Result<InsertionElement<'a>, DatabaseExecutionError> {
    let system = &system_context.system;

    let key_column = key_column_id.get_column(system);
    let argument_value = match &field.relation {
        DatabaseRelation::ManyToOne { other_type_id, .. } => {
            // TODO: Include enough information in the ManyToOne relation to not need this much logic here
            let other_type = &system.database_types[*other_type_id];
            let other_type_pk_field_name = other_type
                .pk_column_id()
                .map(|column_id| &column_id.get_column(system).column_name)
                .ok_or_else(|| {
                    DatabaseExecutionError::Generic(format!(
                        "{} did not have a primary key field when computing many-to-one for {}",
                        other_type.name, field.name
                    ))
                })?;
            match super::get_argument_field(argument, other_type_pk_field_name) {
                Some(other_type_pk_arg) => other_type_pk_arg,
                None => todo!(),
            }
        }
        _ => argument,
    };

    let value_column = cast::literal_column(argument_value, key_column).with_context(format!(
        "While trying to get literal column for {}.{}",
        key_column.table_name, key_column.column_name
    ))?;

    Ok(InsertionElement::SelfInsert(ColumnValuePair::new(
        key_column,
        value_column.into(),
    )))
}

/// Map foreign elements of a data parameter
/// For example, if the data parameter is `data: {name: "venue-name", concerts: [{<concert-info1>}, {<concert-info2>}]} }
/// this needs to be called for the `concerts` part (which is mapped to a separate table)
fn map_foreign<'a>(
    field: &'a DatabaseField,
    argument: &'a ConstValue,
    parent_data_type: &'a DatabaseType,
    system_context: &DatabaseSystemContext<'a>,
) -> Result<InsertionElement<'a>, DatabaseExecutionError> {
    let system = &system_context.system;

    fn underlying_type<'a>(
        data_type: &'a DatabaseType,
        system: &'a ModelDatabaseSystem,
    ) -> &'a DatabaseType {
        // TODO: Unhack this. Most likely, we need to separate input types from output types and have input types carry
        //       additional information (such as the associated model type) so that we can get the id column more directly
        match &data_type.kind {
            DatabaseTypeKind::Primitive => todo!(),
            DatabaseTypeKind::Composite(kind) => {
                &system.database_types[system.queries[kind.pk_query].return_type.type_id]
            }
        }
    }

    let field_type = field.typ.base_type(&system.mutation_types);

    // TODO: Cleanup in the next round

    // Find the column corresponding to the primary key in the parent
    // For example, if the mutation is (assume `Venue -> [Concert]` relation)
    // `createVenue(data: {name: "V1", published: true, concerts: [{title: "C1V1", published: true}, {title: "C1V2", published: false}]})`
    // we need to create a column that evaluates to `select "venues"."id" from "venues"`

    let parent_type = underlying_type(parent_data_type, system);
    let parent_table = &system.tables[parent_type.table_id().unwrap()];

    let parent_pk_physical_column = parent_type.pk_column_id().unwrap().get_column(system);

    // Find the column that the current entity refers to in the parent entity
    // In the above example, this would be "venue_id"
    let self_type = underlying_type(field_type, system);
    let self_table = &system.tables[self_type.table_id().unwrap()];
    let self_reference_column = self_type
        .model_fields()
        .iter()
        .find(|self_field| match self_field.relation.self_column() {
            Some(column_id) => match &column_id.get_column(system).typ {
                payas_sql::PhysicalColumnType::ColumnReference {
                    ref_table_name,
                    ref_column_name,
                    ..
                } => {
                    ref_table_name == &parent_pk_physical_column.table_name
                        && ref_column_name == &parent_pk_physical_column.column_name
                }
                _ => false,
            },
            None => false,
        })
        .unwrap()
        .relation
        .self_column()
        .unwrap()
        .get_column(system);

    let insertion = map_argument(field_type, argument, system_context)?;

    Ok(InsertionElement::NestedInsert(NestedInsertion {
        relation: NestedElementRelation {
            column: self_reference_column,
            table: self_table,
        },
        self_table,
        parent_table,
        insertions: insertion,
    }))
}
