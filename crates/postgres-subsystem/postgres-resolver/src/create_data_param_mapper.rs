use async_graphql_value::ConstValue;

use payas_sql::{
    AbstractInsert, AbstractSelect, ColumnValuePair, InsertionElement, InsertionRow,
    NestedElementRelation, NestedInsertion,
};
use postgres_model::{
    column_id::ColumnId,
    model::ModelPostgresSystem,
    operation::{CreateDataParameter, OperationReturnType},
    relation::PostgresRelation,
    types::{PostgresCompositeType, PostgresField, PostgresType, EntityType},
};

use crate::sql_mapper::SQLMapper;

use super::{
    cast,
    postgres_execution_error::{PostgresExecutionError, WithContext},
};

pub struct InsertOperation<'a> {
    pub data_param: &'a CreateDataParameter,
    pub return_type: &'a OperationReturnType,
    pub select: AbstractSelect<'a>,
}

impl<'a> SQLMapper<'a, AbstractInsert<'a>> for InsertOperation<'a> {
    fn to_sql(
        self,
        argument: &'a ConstValue,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractInsert<'a>, PostgresExecutionError> {
        let table = self.return_type.physical_table(subsystem);

        let data_type = &subsystem.mutation_types[self.data_param.typ.type_id];

        let rows = map_argument(data_type, argument, subsystem)?;

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

pub(crate) fn map_argument<'a>(
    data_type: &'a PostgresCompositeType,
    argument: &'a ConstValue,
    subsystem: &'a ModelPostgresSystem,
) -> Result<Vec<InsertionRow<'a>>, PostgresExecutionError> {
    match argument {
        ConstValue::List(arguments) => arguments
            .iter()
            .map(|argument| map_single(data_type, argument, subsystem))
            .collect(),
        _ => vec![map_single(data_type, argument, subsystem)]
            .into_iter()
            .collect(),
    }
}

/// Map a single item from the data parameter
fn map_single<'a>(
    data_type: &'a PostgresCompositeType,
    argument: &'a ConstValue,
    subsystem: &'a ModelPostgresSystem,
) -> Result<InsertionRow<'a>, PostgresExecutionError> {
    let fields = &data_type.fields;

    let row: Result<Vec<_>, _> = fields
        .iter()
        .flat_map(|field| {
            // Process fields that map to a column in the current table
            let field_self_column = field.relation.self_column();
            let field_arg = super::util::get_argument_field(argument, &field.name);

            field_arg.map(|field_arg| match field_self_column {
                Some(field_self_column) => {
                    map_self_column(field_self_column, field, field_arg, subsystem)
                }
                None => map_foreign(field, field_arg, data_type, subsystem),
            })
        })
        .collect();

    Ok(InsertionRow { elems: row? })
}

fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a PostgresField,
    argument: &'a ConstValue,
    subsystem: &'a ModelPostgresSystem,
) -> Result<InsertionElement<'a>, PostgresExecutionError> {
    let key_column = key_column_id.get_column(subsystem);
    let argument_value = match &field.relation {
        PostgresRelation::ManyToOne { other_type_id, .. } => {
            // TODO: Include enough information in the ManyToOne relation to not need this much logic here
            let other_type = &subsystem.entity_types[*other_type_id];
            let other_type_pk_field_name = other_type
                .pk_column_id()
                .map(|column_id| &column_id.get_column(subsystem).column_name)
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
    field: &'a PostgresField,
    argument: &'a ConstValue,
    parent_data_type: &'a PostgresCompositeType,
    subsystem: &'a ModelPostgresSystem,
) -> Result<InsertionElement<'a>, PostgresExecutionError> {
    fn underlying_type<'a>(
        data_type: &'a EntityType,
        system: &'a ModelPostgresSystem,
    ) -> &'a EntityType {
        let return_type_id = system.pk_queries[data_type.pk_query].return_type.type_id;

        &system.entity_types[return_type_id]
    }

    let field_type = field
        .typ
        .base_type(&subsystem.primitive_types, &subsystem.entity_types);

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
        .get_column(subsystem);

    let insertion = map_argument(field_type, argument, subsystem)?;

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
