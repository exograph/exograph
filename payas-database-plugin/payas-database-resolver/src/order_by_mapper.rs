use async_graphql_value::ConstValue;

use crate::column_path_util::to_column_path;
use payas_database_model::{
    column_path::ColumnIdPath,
    order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeKind},
};
use payas_sql::{AbstractOrderBy, Ordering};

use crate::to_column_id_path;

use super::{database_system_context::DatabaseSystemContext, DatabaseExecutionError};

pub(crate) trait OrderByParameterMapper<'a> {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: Option<ColumnIdPath>,
        system_context: &DatabaseSystemContext<'a>,
    ) -> Result<AbstractOrderBy<'a>, DatabaseExecutionError>;
}

impl<'a> OrderByParameterMapper<'a> for OrderByParameter {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: Option<ColumnIdPath>,
        system_context: &DatabaseSystemContext<'a>,
    ) -> Result<AbstractOrderBy<'a>, DatabaseExecutionError> {
        let parameter_type = &system_context.system.order_by_types[self.typ.type_id];
        fn flatten<E>(order_bys: Result<Vec<AbstractOrderBy>, E>) -> Result<AbstractOrderBy, E> {
            let mapped = order_bys?.into_iter().flat_map(|elem| elem.0).collect();
            Ok(AbstractOrderBy(mapped))
        }

        match argument {
            ConstValue::Object(elems) => {
                let mapped = elems
                    .iter()
                    .map(|elem| {
                        order_by_pair(
                            parameter_type,
                            elem.0,
                            elem.1,
                            parent_column_path.clone(),
                            system_context,
                        )
                    })
                    .collect();

                flatten(mapped)
            }
            ConstValue::List(elems) => {
                let mapped = elems
                    .iter()
                    .map(|elem| {
                        self.map_to_order_by(elem, parent_column_path.clone(), system_context)
                    })
                    .collect();

                flatten(mapped)
            }

            _ => todo!(), // Invalid
        }
    }
}

fn order_by_pair<'a>(
    typ: &'a OrderByParameterType,
    parameter_name: &str,
    parameter_value: &'a ConstValue,
    parent_column_path: Option<ColumnIdPath>,
    system_context: &DatabaseSystemContext<'a>,
) -> Result<AbstractOrderBy<'a>, DatabaseExecutionError> {
    let parameter = match &typ.kind {
        OrderByParameterTypeKind::Composite { parameters } => {
            match parameters.iter().find(|p| p.name == parameter_name) {
                Some(parameter) => Ok(parameter),
                None => Err(DatabaseExecutionError::Validation(format!(
                    "Invalid order by parameter {parameter_name}"
                ))),
            }
        }
        _ => Err(DatabaseExecutionError::Validation(
            "Invalid primitive order by parameter".to_string(),
        )),
    }?;

    // If this is a leaf node ({something: ASC} kind), then resolve the ordering. If not, then recurse with a new parent column path.
    // TODO: This feels a bit of a hack (we need a better way to find if this is a leaf parameter). Revisit this after we have a improved validation (#483)
    if &parameter.type_name == "Ordering" {
        let new_column_path = to_column_path(
            &parent_column_path,
            &parameter.column_path_link,
            system_context.system,
        );
        ordering(parameter_value).map(|ordering| AbstractOrderBy(vec![(new_column_path, ordering)]))
    } else {
        let new_parent_column_path =
            to_column_id_path(&parent_column_path, &parameter.column_path_link);
        parameter.map_to_order_by(parameter_value, new_parent_column_path, system_context)
    }
}

fn ordering(argument: &ConstValue) -> Result<Ordering, DatabaseExecutionError> {
    fn str_ordering(value: &str) -> Result<Ordering, DatabaseExecutionError> {
        if value == "ASC" {
            Ok(Ordering::Asc)
        } else if value == "DESC" {
            Ok(Ordering::Desc)
        } else {
            Err(DatabaseExecutionError::Generic(format!(
                "Cannot match {} as valid ordering",
                value
            )))
        }
    }

    match argument {
        ConstValue::Enum(value) => str_ordering(value.as_str()),
        ConstValue::String(value) => str_ordering(value.as_str()), // Needed when processing values from variables (that don't get mapped to the Enum type)
        arg => Err(DatabaseExecutionError::Generic(format!(
            "Unable to process ordering argument {}",
            arg
        ))), // return an error
    }
}
