use crate::sql::order::{OrderBy, Ordering};
use anyhow::*;
use async_graphql_value::Value;
use payas_model::model::order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeKind};
use payas_model::sql::column::PhysicalColumn;

use super::{operation_context::OperationContext, operation_mapper::SQLMapper};

impl<'a> SQLMapper<'a, OrderBy<'a>> for OrderByParameter {
    fn map_to_sql(
        &self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<OrderBy<'a>> {
        let parameter_type = &operation_context
            .query_context
            .executor
            .system
            .order_by_types[self.type_id];
        parameter_type.map_to_sql(argument, operation_context)
    }
}

impl<'a> SQLMapper<'a, OrderBy<'a>> for OrderByParameterType {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<OrderBy<'a>> {
        match argument {
            Value::Object(elems) => {
                // TODO: Reject elements with multiple elements (see the comment in query.rs)
                let mapped: Vec<_> = elems
                    .iter()
                    .map(|elem| order_by_pair(self, elem.0, elem.1, operation_context))
                    .collect::<Result<Vec<_>>>()?;
                Ok(OrderBy(mapped))
            }
            Value::List(elems) => {
                let mapped: Vec<_> = elems
                    .iter()
                    .map(|elem| self.map_to_sql(elem, operation_context))
                    .collect::<Result<Vec<_>>>()
                    .with_context(|| {
                        format!(
                            "While mapping list elements to SQL for parameter {}",
                            self.name
                        )
                    })?
                    .into_iter()
                    .flat_map(|elem| elem.0)
                    .collect();
                Ok(OrderBy(mapped))
            }
            Value::Variable(name) => {
                let resolved = operation_context.resolve_variable(name.as_str());
                self.map_to_sql(resolved.unwrap(), operation_context)
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
) -> Result<(&'a PhysicalColumn, Ordering)> {
    let parameter = match &typ.kind {
        OrderByParameterTypeKind::Composite { parameters } => {
            parameters.iter().find(|p| p.name == parameter_name)
        }
        _ => None,
    };

    let column_id = parameter.as_ref().and_then(|p| p.column_id.as_ref());

    let column = column_id
        .unwrap()
        .get_column(operation_context.query_context.executor.system);

    ordering(parameter_value).map(|ordering| (column, ordering))
}

fn ordering(argument: &Value) -> Result<Ordering> {
    fn str_ordering(value: &str) -> Result<Ordering> {
        if value == "ASC" {
            Ok(Ordering::Asc)
        } else if value == "DESC" {
            Ok(Ordering::Desc)
        } else {
            bail!("Cannot match {} as valid ordering", value) // return an error
        }
    }

    match argument {
        Value::Enum(value) => str_ordering(value.as_str()),
        Value::String(value) => str_ordering(value.as_str()), // Needed when processing values from variables (that don't get mapped to the Enum type)
        arg => bail!("Unable to process ordering argument {}", arg), // return an error
    }
}
