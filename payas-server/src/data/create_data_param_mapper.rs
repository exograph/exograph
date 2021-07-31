use std::collections::{HashMap, HashSet};

use async_graphql_value::Value;

use crate::sql::column::Column;

use payas_model::{
    model::{
        column_id::ColumnId, relation::GqlRelation, types::GqlTypeKind, GqlCompositeTypeKind,
        GqlField, GqlType,
    },
    sql::{column::PhysicalColumn, PhysicalTable, SQLOperation},
};

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

#[derive(Debug)]
struct SingleInsertion<'a> {
    pub self_row: HashMap<&'a PhysicalColumn, &'a Column<'a>>,
    pub nested_rows: Vec<InsertionInfo<'a>>,
}

#[derive(Debug)]
pub struct InsertionInfo<'a> {
    pub table: &'a PhysicalTable,
    pub columns: Vec<&'a PhysicalColumn>,
    pub values: Vec<Vec<&'a Column<'a>>>,
    pub nested: Vec<InsertionInfo<'a>>,
}

impl<'a> InsertionInfo<'a> {
    /// Compute a combined set of operations considering nested insertions
    pub fn operation(
        self,
        operation_context: &'a OperationContext<'a>,
    ) -> Vec<(String, SQLOperation<'a>)> {
        let InsertionInfo {
            table,
            columns,
            values,
            nested,
        } = self;

        let main_insertion = (
            table.name.clone(),
            SQLOperation::Insert(table.insert(
                columns,
                values,
                vec![operation_context.create_column(Column::Star)],
            )),
        );

        let mut ops = Vec::with_capacity(&nested.len() + 1);
        ops.push(main_insertion);

        let nested_insertions = nested
            .into_iter()
            .flat_map(|item| item.operation(operation_context));

        ops.extend(nested_insertions);
        ops
    }
}

impl<'a> SQLMapper<'a, InsertionInfo<'a>> for GqlType {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> InsertionInfo<'a> {
        let table = self
            .table_id()
            .map(|table_id| &operation_context.query_context.system.tables[table_id])
            .unwrap();

        match argument {
            Value::List(elems) => {
                let unaligned: Vec<_> = elems
                    .iter()
                    .map(|elem| map_single(self, elem, operation_context))
                    .collect();

                align(unaligned, table)
            }
            _ => {
                let raw = map_single(self, argument, operation_context);
                let (columns, values) = raw.self_row.into_iter().unzip();
                InsertionInfo {
                    table,
                    columns,
                    values: vec![values],
                    nested: raw.nested_rows,
                }
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

    for item in unaligned.into_iter() {
        let mut row = Vec::with_capacity(keys_count);
        for key in &all_keys {
            let value = item.self_row.get(key).copied().unwrap_or(&Column::Null);
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
/// Specifically, either the whole of a single insert one of the element of multiuple inserts
fn map_single<'a>(
    typ: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> SingleInsertion<'a> {
    let argument = match argument {
        Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
        _ => argument,
    };

    let fields = match &typ.kind {
        GqlTypeKind::Primitive => panic!("Query attempted on a primitive type"),
        GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => fields,
    };

    let mut self_row = HashMap::new();
    let mut nested_rows = Vec::new();

    fields.iter().for_each(|field| {
        // Process fields that map to a column in the current table
        let field_self_column = field.relation.self_column();
        let field_arg = operation_context.get_argument_field(argument, &field.name);

        match field_arg {
            Some(field_arg) => match field_self_column {
                Some(field_self_column) => {
                    let (col, value) =
                        map_self_column(field_self_column, field, field_arg, operation_context);
                    self_row.insert(col, value);
                }
                None => nested_rows.push(map_foreign(field, field_arg, operation_context)),
            },
            None => (), // TODO: Report an error if the field is non-optional
        }
    });

    SingleInsertion {
        self_row,
        nested_rows,
    }
}

fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a GqlField,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> (&'a PhysicalColumn, &'a Column<'a>) {
    let system = &operation_context.query_context.system;

    let key_column = key_column_id.get_column(system);
    let argument_value = match &field.relation {
        GqlRelation::ManyToOne { other_type_id, .. } => {
            // TODO: Include enough information in the ManyToOne relation to not need this much logic here
            let other_type = &system.types[*other_type_id];
            let other_type_pk_field_name = other_type
                .pk_column_id()
                .map(|column_id| &column_id.get_column(system).column_name)
                .unwrap();
            match operation_context.get_argument_field(argument, other_type_pk_field_name) {
                Some(other_type_pk_arg) => other_type_pk_arg,
                None => todo!(),
            }
        }
        _ => argument,
    };
    let value_column = operation_context.literal_column(argument_value.clone(), key_column);
    (key_column, value_column)
}

/// Map foreign elements of a data parameter
/// For example, if the data parameter is `data: {name: "venue-name", concerts: [{<concert-info1>}, {<concert-info1>}]} }
/// this needs to be called for the `concerts` part (which is mapped to a separate table)
fn map_foreign<'a>(
    field: &'a GqlField,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> InsertionInfo<'a> {
    let system = &operation_context.query_context.system;

    let field_type = field.typ.base_type(&system.mutation_types);

    field_type.map_to_sql(argument, operation_context)
}
