use crate::execution::operations_context::OperationsContext;
use anyhow::{bail, Context, Result};
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
        query_context: &'a OperationsContext<'a>,
    ) -> Result<AbstractOrderBy<'a>>;
}

impl<'a> OrderByParameterMapper<'a> for OrderByParameter {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: &'a Option<ColumnIdPath>,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<AbstractOrderBy<'a>> {
        let parameter_type = &query_context.get_system().order_by_types[self.type_id];

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
                            query_context,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(AbstractOrderBy(mapped))
            }
            ConstValue::List(elems) => {
                let mapped: Vec<_> = elems
                    .iter()
                    .map(|elem| self.map_to_order_by(elem, parent_column_path, query_context))
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
    query_context: &'a OperationsContext<'a>,
) -> Result<(ColumnPath<'a>, Ordering)> {
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
        query_context.get_system(),
    );

    ordering(parameter_value).map(|ordering| (new_column_path, ordering))
}

fn ordering(argument: &ConstValue) -> Result<Ordering> {
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
        ConstValue::Enum(value) => str_ordering(value.as_str()),
        ConstValue::String(value) => str_ordering(value.as_str()), // Needed when processing values from variables (that don't get mapped to the Enum type)
        arg => bail!("Unable to process ordering argument {}", arg), // return an error
    }
}
