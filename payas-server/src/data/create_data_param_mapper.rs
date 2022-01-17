use std::collections::{HashMap, HashSet};

use anyhow::*;
use async_graphql_value::ConstValue;
use maybe_owned::MaybeOwned;

use crate::{execution::query_context::QueryContext, sql::column::Column};

use payas_model::{
    model::{
        column_id::ColumnId, relation::GqlRelation, system::ModelSystem, types::GqlTypeKind,
        GqlCompositeType, GqlField, GqlType,
    },
    sql::{
        column::PhysicalColumn, predicate::Predicate, Limit, Offset, PhysicalTable, SQLOperation,
        TableQuery,
    },
};

use super::operation_mapper::SQLMapper;

#[derive(Debug)]
struct SingleInsertion<'a> {
    pub self_row: HashMap<&'a PhysicalColumn, Column<'a>>,
    pub nested_rows: Vec<InsertionInfo<'a>>,
}

#[derive(Debug)]
pub struct InsertionInfo<'a> {
    pub table: &'a PhysicalTable,
    pub columns: Vec<&'a PhysicalColumn>,
    pub values: Vec<Vec<MaybeOwned<'a, Column<'a>>>>,
    pub nested: Vec<InsertionInfo<'a>>,
}

impl<'a> InsertionInfo<'a> {
    /// Compute a combined set of operations considering nested insertions
    pub fn operation(
        self,
        query_context: &'a QueryContext<'a>,
        return_data: bool,
    ) -> Vec<(String, SQLOperation<'a>)> {
        let InsertionInfo {
            table,
            columns,
            values,
            nested,
        } = self;

        let returning = if return_data {
            vec![Column::Star.into()]
        } else {
            vec![]
        };
        let main_insertion = (
            table.name.clone(),
            SQLOperation::Insert(table.insert(columns, values, returning)),
        );

        let mut ops = Vec::with_capacity(&nested.len() + 1);
        ops.push(main_insertion);

        let nested_insertions = nested
            .into_iter()
            .flat_map(|item| item.operation(query_context, return_data));

        ops.extend(nested_insertions);
        ops
    }
}

impl<'a> SQLMapper<'a, InsertionInfo<'a>> for GqlType {
    fn map_to_sql(
        &'a self,
        argument: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<InsertionInfo<'a>> {
        let table = self
            .table_id()
            .map(|table_id| &query_context.get_system().tables[table_id])
            .unwrap();

        match argument {
            ConstValue::List(elems) => {
                let unaligned = elems
                    .iter()
                    .enumerate()
                    .map(|(index, elem)| map_single(self, elem, Some(index), query_context))
                    .collect::<Result<Vec<_>>>()?;

                Ok(align(unaligned, table))
            }
            _ => {
                let raw = map_single(self, argument, None, query_context)?;
                let (columns, values) =
                    raw.self_row.into_iter().map(|(c, v)| (c, v.into())).unzip();
                Ok(InsertionInfo {
                    table,
                    columns,
                    values: vec![values],
                    nested: raw.nested_rows,
                })
            }
        }
    }
}

/// Align multiple SingleInsertion's to account for misaligned and missing columns
/// For example, if the input is {data: [{a: 1, b: 2}, {a: 3, c: 4}]}, we will have the 'a' key in both
/// but only 'b' or 'c' keys in others. So we need align columns that can be supplied to an insert statement
/// (a, b, c), [(1, 2, null), (3, null, 4)]
fn align<'a>(unaligned: Vec<SingleInsertion<'a>>, table: &'a PhysicalTable) -> InsertionInfo<'a> {
    let mut all_keys = HashSet::new();
    for item in unaligned.iter() {
        all_keys.extend(item.self_row.keys())
    }

    let keys_count = all_keys.len();

    let mut values = Vec::with_capacity(unaligned.len());
    let mut nested = vec![];

    for mut item in unaligned.into_iter() {
        let mut row = Vec::with_capacity(keys_count);
        for key in &all_keys {
            let value = item
                .self_row
                .remove(key)
                .map(|v| v.into())
                .unwrap_or_else(|| Column::Null.into());
            row.push(value);
        }

        values.push(row);
        nested.extend(item.nested_rows);
    }

    InsertionInfo {
        table,
        columns: all_keys.into_iter().collect(),
        values,
        nested,
    }
}

/// Map a single item from the data parameter
/// Specifically, either the whole of a single insert one of the element of multiple inserts
fn map_single<'a>(
    input_data_type: &'a GqlType,
    argument: &'a ConstValue,
    index: Option<usize>, // Index if the multiple entries are being inserted (such as createVenues (note the plural form))
    query_context: &'a QueryContext<'a>,
) -> Result<SingleInsertion<'a>> {
    let fields = match &input_data_type.kind {
        GqlTypeKind::Primitive => bail!("Query attempted on a primitive type"),
        GqlTypeKind::Composite(GqlCompositeType { fields, .. }) => fields,
    };

    let mut self_row = HashMap::new();
    let mut nested_rows_results = Vec::new();

    for field in fields.iter() {
        // Process fields that map to a column in the current table
        let field_self_column = field.relation.self_column();
        let field_arg = query_context.get_argument_field(argument, &field.name);

        if let Some(field_arg) = field_arg {
            match field_self_column {
                Some(field_self_column) => {
                    let (col, value) =
                        map_self_column(field_self_column, field, field_arg, query_context)?;
                    self_row.insert(col, value);
                }
                None => nested_rows_results.push(map_foreign(
                    field,
                    field_arg,
                    index,
                    input_data_type,
                    query_context,
                )),
            } // TODO: Report an error if the field is non-optional and the if-let doesn't match
        }
    }

    let nested_rows = nested_rows_results
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .context("While mapping foreign elements as nested rows")?;

    Ok(SingleInsertion {
        self_row,
        nested_rows,
    })
}

fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a GqlField,
    argument: &'a ConstValue,
    query_context: &'a QueryContext<'a>,
) -> Result<(&'a PhysicalColumn, Column<'a>)> {
    let system = query_context.get_system();

    let key_column = key_column_id.get_column(system);
    let argument_value = match &field.relation {
        GqlRelation::ManyToOne { other_type_id, .. } => {
            // TODO: Include enough information in the ManyToOne relation to not need this much logic here
            let other_type = &system.types[*other_type_id];
            let other_type_pk_field_name = other_type
                .pk_column_id()
                .map(|column_id| &column_id.get_column(system).column_name)
                .ok_or_else(|| {
                    anyhow!(
                        "{} did not have a primary key field when computing many-to-one for {}",
                        other_type.name,
                        field.name
                    )
                })?;
            match query_context.get_argument_field(argument, other_type_pk_field_name) {
                Some(other_type_pk_arg) => other_type_pk_arg,
                None => todo!(),
            }
        }
        _ => argument,
    };

    let value_column = query_context
        .literal_column(argument_value, key_column)
        .with_context(|| {
            format!(
                "While trying to get literal column for {}.{}",
                key_column.table_name, key_column.column_name
            )
        })?;

    Ok((key_column, value_column))
}

/// Map foreign elements of a data parameter
/// For example, if the data parameter is `data: {name: "venue-name", concerts: [{<concert-info1>}, {<concert-info2>}]} }
/// this needs to be called for the `concerts` part (which is mapped to a separate table)
fn map_foreign<'a>(
    field: &'a GqlField,
    argument: &'a ConstValue,
    parent_index: Option<usize>,
    parent_data_type: &'a GqlType,
    query_context: &'a QueryContext<'a>,
) -> Result<InsertionInfo<'a>> {
    let system = query_context.get_system();

    fn underlying_type<'a>(data_type: &'a GqlType, system: &'a ModelSystem) -> &'a GqlType {
        // TODO: Unhack this. Most likely, we need to separate input types from output types and have input types carry
        //       additional information (such as the associated model type) so that we can get the id column more directly
        match &data_type.kind {
            GqlTypeKind::Primitive => todo!(),
            GqlTypeKind::Composite(kind) => {
                &system.types[system.queries[kind.get_pk_query()].return_type.type_id]
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
    let parent_physical_table = &system.tables[parent_type.table_id().unwrap()];
    let parent_pk_physical_column = parent_type.pk_column_id().unwrap().get_column(system);

    fn create_select<'a>(
        parent_table: TableQuery<'a>,
        parent_pk_physical_column: &'a PhysicalColumn,
        parent_index: Option<usize>,
    ) -> Column<'a> {
        Column::SelectionTableWrapper(Box::new(parent_table.select(
            vec![Column::Physical(parent_pk_physical_column).into()],
            Predicate::True,
            None,
            parent_index.map(|index| Offset(index as i64)),
            parent_index.map(|_| Limit(1)),
            false,
        )))
    }

    // Find the column that the current entity refers to in the parent entity
    // In the above example, this would be "venue_id"
    let self_type = underlying_type(field_type, system);
    let self_reference_column = self_type
        .model_fields()
        .iter()
        .find(|self_field| match self_field.relation.self_column() {
            Some(column_id) => match &column_id.get_column(system).typ {
                payas_model::sql::column::PhysicalColumnType::ColumnReference {
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

    // First map the user-specified information (arguments)
    let InsertionInfo {
        table,
        mut columns,
        mut values,
        nested,
    } = field_type.map_to_sql(argument, query_context)?;

    // Then, push the information to have the nested entity refer to the parent entity
    columns.push(self_reference_column);

    values.iter_mut().for_each(|value| {
        let parent_table = TableQuery::Physical(parent_physical_table);
        value.push(create_select(parent_table, parent_pk_physical_column, parent_index).into())
    });

    Ok(InsertionInfo {
        table,
        columns,
        values,
        nested,
    })
}
