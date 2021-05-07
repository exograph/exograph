use crate::{model::order::*, sql::column::Column};

use crate::sql::order::{OrderBy, Ordering};
use async_graphql_value::Value;

use super::operation_context::OperationContext;

impl OrderByParameter {
    pub fn compute_order_by<'a>(
        &self,
        argument: &Value,
        operation_context: &'a OperationContext<'a>,
    ) -> OrderBy<'a> {
        let parameter_type = &operation_context.query_context.system.order_by_types[self.type_id];
        parameter_type.compute_order_by(argument, operation_context)
    }
}

impl OrderByParameterType {
    pub fn compute_order_by<'a>(
        &self,
        argument: &Value,
        operation_context: &'a OperationContext<'a>,
    ) -> OrderBy<'a> {
        match argument {
            Value::Object(elems) => {
                // TODO: Reject elements with multiple elements (see the comment in query.rs)
                let mapped: Vec<(&'a Column<'a>, Ordering)> = elems
                    .iter()
                    .map(|elem| self.order_by_pair(elem.0, elem.1, operation_context))
                    .collect();
                OrderBy(mapped)
            }
            Value::List(elems) => {
                let mapped: Vec<(&'a Column<'a>, Ordering)> = elems
                    .iter()
                    .flat_map(|elem| self.compute_order_by(elem, operation_context).0)
                    .collect();
                OrderBy(mapped)
            }
            _ => todo!(), // Invalid
        }
    }

    fn order_by_pair<'a>(
        &self,
        parameter_name: &str,
        parameter_value: &Value,
        operation_context: &'a OperationContext<'a>,
    ) -> (&'a Column<'a>, Ordering) {
        let parameter = match &self.kind {
            OrderByParameterTypeKind::Composite { parameters } => {
                parameters.iter().find(|p| p.name == parameter_name)
            }
            _ => None,
        };

        let column_id = parameter.as_ref().and_then(|p| p.column_id.as_ref());

        let column = operation_context.create_column_with_id(&column_id.unwrap());

        (column, Self::ordering(parameter_value))
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
}
