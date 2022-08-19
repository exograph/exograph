use async_graphql_value::ConstValue;

use payas_model::model::{
    order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeKind},
    predicate::ColumnIdPath,
};
use payas_resolver_core::column_path_util::to_column_path;
use payas_sql::{AbstractOrderBy, ColumnPath, Ordering};

use super::{
    database_execution_error::WithContext, database_system_context::DatabaseSystemContext,
    DatabaseExecutionError,
};

pub(crate) trait OrderByParameterMapper<'a> {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: &'a Option<ColumnIdPath>,
        system_context: &DatabaseSystemContext<'a>,
    ) -> Result<AbstractOrderBy<'a>, DatabaseExecutionError>;
}

impl<'a> OrderByParameterMapper<'a> for OrderByParameter {
    fn map_to_order_by(
        &'a self,
        argument: &'a ConstValue,
        parent_column_path: &'a Option<ColumnIdPath>,
        system_context: &DatabaseSystemContext<'a>,
    ) -> Result<AbstractOrderBy<'a>, DatabaseExecutionError> {
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
    system_context: &DatabaseSystemContext<'a>,
) -> Result<(ColumnPath<'a>, Ordering), DatabaseExecutionError> {
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
        system_context.system,
    );

    ordering(parameter_value).map(|ordering| (new_column_path, ordering))
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
