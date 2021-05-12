use crate::sql::column::Column;

use crate::sql::order::{OrderBy, Ordering};
use async_graphql_value::Value;
use payas_model::model::order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeKind};

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

impl<'a> SQLMapper<'a, OrderBy<'a>> for OrderByParameter {
    fn map_to_sql(
        &self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> OrderBy<'a> {
        let parameter_type = &operation_context.query_context.system.order_by_types[self.type_id];
        parameter_type.map_to_sql(argument, operation_context)
    }
}

impl<'a> SQLMapper<'a, OrderBy<'a>> for OrderByParameterType {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> OrderBy<'a> {
        match argument {
            Value::Object(elems) => {
                // TODO: Reject elements with multiple elements (see the comment in query.rs)
                let mapped: Vec<(&'a Column<'a>, Ordering)> = elems
                    .iter()
                    .map(|elem| order_by_pair(self, elem.0, elem.1, operation_context))
                    .collect();
                OrderBy(mapped)
            }
            Value::List(elems) => {
                let mapped: Vec<(&'a Column<'a>, Ordering)> = elems
                    .iter()
                    .flat_map(|elem| self.map_to_sql(elem, operation_context).0)
                    .collect();
                OrderBy(mapped)
            }
            _ => todo!(), // Invalid
        }
    }
}

fn order_by_pair<'a>(
    typ: &'a OrderByParameterType,
    parameter_name: &str,
    parameter_value: &Value,
    operation_context: &'a OperationContext<'a>,
) -> (&'a Column<'a>, Ordering) {
    let parameter = match &typ.kind {
        OrderByParameterTypeKind::Composite { parameters } => {
            parameters.iter().find(|p| p.name == parameter_name)
        }
        _ => None,
    };

    let column_id = parameter.as_ref().and_then(|p| p.column_id.as_ref());

    let column = operation_context.create_column_with_id(&column_id.unwrap());

    (column, ordering(parameter_value))
}

fn ordering(argument: &Value) -> Ordering {
    match argument {
        Value::Enum(value) => {
            if value.as_str() == "ASC" {
                Ordering::Asc
            } else if value.as_str() == "DESC" {
                Ordering::Desc
            } else {
                todo!() // return an error
            }
        }
        _ => todo!(), // return an error
    }
}
