use std::collections::HashMap;

use async_graphql_value::Value;

use crate::sql::column::Column;

use payas_model::{
    model::{
        column_id::ColumnId, operation::CreateDataParameter, relation::GqlRelation,
        types::GqlTypeKind, GqlCompositeTypeKind, GqlField, GqlType,
    },
    sql::column::PhysicalColumn,
};

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

#[derive(Debug)]
pub struct InsertionRow<'a> {
    pub column_values: HashMap<&'a PhysicalColumn, &'a Column<'a>>,
}

impl<'a> SQLMapper<'a, InsertionRow<'a>> for CreateDataParameter {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> InsertionRow<'a> {
        let system = &operation_context.query_context.system;
        let model_type = &system.mutation_types[self.type_id];

        model_type.map_to_sql(argument, operation_context)
    }
}

impl<'a> SQLMapper<'a, InsertionRow<'a>> for GqlType {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> InsertionRow<'a> {
        let argument = match argument {
            Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
            _ => argument,
        };

        let row = match &self.kind {
            GqlTypeKind::Primitive => panic!(),
            GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => fields
                .iter()
                .flat_map(|field| {
                    println!("\n---------");
                    dbg!(&field.name);
                    dbg!(argument);
                    dbg!(operation_context.get_argument_field(argument, &field.name));
                    match operation_context.get_argument_field(argument, &field.name) {
                        Some(argument) => match field.relation.self_column() {
                            Some(key_column_id) => {
                                map_self_column(key_column_id, field, argument, operation_context)
                            }
                            None => {
                                map_contained(field, argument, operation_context);
                                None
                            }
                        },
                        None => None,
                    }
                })
                .collect(),
        };

        InsertionRow { column_values: row }
    }
}

fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a GqlField,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> Option<(&'a PhysicalColumn, &'a Column<'a>)> {
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
    Some((key_column, value_column))
}

fn map_contained<'a>(
    field: &'a GqlField,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> Option<()> {
    println!("map_contained {:?} {:?}", field, argument);
    let system = &operation_context.query_context.system;

    let field_type = field.typ.base_type(&system.mutation_types);

    println!("{}", argument);
    println!("{:?}", field_type);
    let x = field_type.map_to_sql(argument, operation_context);

    println!("{:?}", x);

    // println!("{:?}", field);

    None
}
