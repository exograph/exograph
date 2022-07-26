use crate::graphql::execution::system_context::SystemContext;
use crate::graphql::execution_error::{ExecutionError, WithContext};
use async_graphql_value::ConstValue;
use payas_model::model::order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeKind};
use payas_model::model::predicate::ColumnIdPath;
use payas_sql::{AbstractOrderBy, ColumnPath, Ordering};

use super::to_column_path;

pub trait OrderByParameterMapper<'a> {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: &'a Option<ColumnIdPath>,
        system_context: &'a SystemContext,
    ) -> Result<AbstractOrderBy<'a>, ExecutionError>;
}

impl<'a> OrderByParameterMapper<'a> for OrderByParameter {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: &'a Option<ColumnIdPath>,
        system_context: &'a SystemContext,
    ) -> Result<AbstractOrderBy<'a>, ExecutionError> {
        let parameter_type = &system_context.system.order_by_types[self.type_id];

        match argument {
            ConstValue::Object(elems) => {
                // TODO: Reject elements with multiple elements (see the comment in query.rs)
                let mapped: Vec<_> = elems
                    .iter()
                    .map(|elem| {
                        order_by_pair(
                            parameter_type,
                            elem.0,
                            elem.1,
                            parent_column_path,
                            system_context,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(AbstractOrderBy(mapped))
            }
            ConstValue::List(elems) => {
                let mapped: Vec<_> = elems
                    .iter()
                    .map(|elem| self.map_to_order_by(elem, parent_column_path, system_context))
                    .collect::<Result<Vec<_>, _>>()
                    .with_context(format!(
                        "While mapping list elements to SQL for parameter {}",
                        self.name
                    ))?
                    .into_iter()
                    .flat_map(|elem| elem.0)
                    .collect();
                Ok(AbstractOrderBy(mapped))
            }

            _ => todo!(), // Invalid
        }
    }
}

fn order_by_pair<'a>(
    typ: &'a OrderByParameterType,
    parameter_name: &str,
    parameter_value: &ConstValue,
    parent_column_path: &Option<ColumnIdPath>,
    system_context: &'a SystemContext,
) -> Result<(ColumnPath<'a>, Ordering), ExecutionError> {
    let parameter = match &typ.kind {
        OrderByParameterTypeKind::Composite { parameters } => {
            parameters.iter().find(|p| p.name == parameter_name)
        }
        _ => None,
    };

    let next_column_id_path_link =
        parameter.and_then(|parameter| parameter.column_path_link.clone());

    let new_column_path = to_column_path(
        parent_column_path,
        &next_column_id_path_link,
        &system_context.system,
    );

    ordering(parameter_value).map(|ordering| (new_column_path, ordering))
}

fn ordering(argument: &ConstValue) -> Result<Ordering, ExecutionError> {
    fn str_ordering(value: &str) -> Result<Ordering, ExecutionError> {
        if value == "ASC" {
            Ok(Ordering::Asc)
        } else if value == "DESC" {
            Ok(Ordering::Desc)
        } else {
            Err(ExecutionError::Generic(format!(
                "Cannot match {} as valid ordering",
                value
            )))
        }
    }

    match argument {
        ConstValue::Enum(value) => str_ordering(value.as_str()),
        ConstValue::String(value) => str_ordering(value.as_str()), // Needed when processing values from variables (that don't get mapped to the Enum type)
        arg => Err(ExecutionError::Generic(format!(
            "Unable to process ordering argument {}",
            arg
        ))), // return an error
    }
}
