use std::collections::{HashMap, HashSet};

use async_graphql_value::Value;

use crate::sql::column::Column;

use payas_model::{
    model::{
        column_id::ColumnId, relation::GqlRelation, types::GqlTypeKind, GqlCompositeTypeKind,
        GqlField, GqlType,
    },
    sql::column::PhysicalColumn,
};

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

#[derive(Debug)]
pub struct InsertionRow<'a> {
    pub column_values: HashMap<&'a PhysicalColumn, &'a Column<'a>>,
}

pub struct InsertionInfo<'a> {
    pub name: String, // A name suitable to be used as the CTE name
    pub columns: Vec<&'a PhysicalColumn>,
    pub values: Vec<Vec<&'a Column<'a>>>,
}

impl<'a> SQLMapper<'a, (Vec<&'a PhysicalColumn>, Vec<Vec<&'a Column<'a>>>)> for GqlType {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> (Vec<&'a PhysicalColumn>, Vec<Vec<&'a Column<'a>>>) {
        match argument {
            Value::List(elems) => {
                let unaligned: Vec<_> = elems
                    .iter()
                    .map(|elem| map_single(self, elem, operation_context))
                    .collect();

                align(unaligned)
            }
            _ => {
                let raw = map_single(self, argument, operation_context).column_values;
                let raw: (Vec<_>, Vec<_>) = raw.into_iter().unzip();
                (raw.0, vec![raw.1])
            }
        }
    }
}

fn align<'a>(
    unaligned: Vec<InsertionRow<'a>>,
) -> (Vec<&'a PhysicalColumn>, Vec<Vec<&'a Column<'a>>>) {
    // Here we may have each mapped element with potentially different set of columns.
    // For example, if the input is {data: [{a: 1, b: 2}, {a: 3, c: 4}]}, we will have the 'a' key in both
    // but only 'b' or 'c' keys in others. So we need align columns that can be supplied to an insert statement
    // (a, b, c), [(1, 2, null), (3, null, 4)]
    let mut all_keys = HashSet::new();
    for item in unaligned.iter() {
        all_keys.extend(item.column_values.keys())
    }

    let keys_count = all_keys.len();

    let mut result = Vec::with_capacity(unaligned.len());
    for item in unaligned.into_iter() {
        let mut row = Vec::with_capacity(keys_count);
        for key in &all_keys {
            let value = item
                .column_values
                .get(key)
                .copied()
                .unwrap_or(&Column::Null);
            row.push(value);
        }

        result.push(row);
    }

    (all_keys.into_iter().collect(), result)
}

/// Map a single item from the data parameter
fn map_single<'a>(
    typ: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> InsertionRow<'a> {
    let argument = match argument {
        Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
        _ => argument,
    };

    let fields = match &typ.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => fields,
    };

    let mut self_row = HashMap::new();
    let mut other_rows = Vec::new();

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
                None => other_rows.push(map_contained(field, field_arg, operation_context)),
            },
            None => (), // TODO: Report an error if the field is non-optional
        }
    });

    println!("{:?}", other_rows);

    InsertionRow {
        column_values: self_row,
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

fn map_contained<'a>(
    field: &'a GqlField,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> (Vec<&'a PhysicalColumn>, Vec<Vec<&'a Column<'a>>>) {
    let system = &operation_context.query_context.system;

    let field_type = field.typ.base_type(&system.mutation_types);

    field_type.map_to_sql(argument, operation_context)
}
