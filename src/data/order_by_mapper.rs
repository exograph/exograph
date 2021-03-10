use crate::sql::table::PhysicalTable;
use crate::{
    model::{order::*, system::ModelSystem},
    sql::column::Column,
};

use crate::introspection::definition::parameter::Parameter;
use crate::sql::order::{OrderBy, Ordering};
use graphql_parser::schema::Value;

impl OrderByParameter {
    pub fn compute_order_by<'a>(
        &self,
        argument: &Value<String>,
        table: &'a PhysicalTable,
        system: &ModelSystem,
    ) -> OrderBy<'a> {
        let parameter_type = system
            .parameter_types
            .find_order_by_parameter_type(&self.type_name)
            .unwrap();
        parameter_type.compute_order_by(argument, table, system)
    }
}

impl OrderByParameterType {
    pub fn compute_order_by<'a>(
        &self,
        argument: &Value<String>,
        table: &'a PhysicalTable,
        system: &ModelSystem,
    ) -> OrderBy<'a> {
        match argument {
            Value::Object(elems) => {
                // TODO: Reject elements with multiple elements (see the comment in query.rs)
                let mapped: Vec<(&'a Column<'a>, Ordering)> = elems
                    .iter()
                    .map(|elem| self.order_by_pair(table, elem.0, elem.1))
                    .collect();
                OrderBy(mapped)
            }
            Value::List(elems) => {
                let mapped: Vec<(&'a Column<'a>, Ordering)> = elems
                    .iter()
                    .flat_map(|elem| self.compute_order_by(elem, table, system).0)
                    .collect();
                OrderBy(mapped)
            }
            _ => todo!(), // Invalid
        }
    }

    fn order_by_pair<'a>(
        &self,
        table: &'a PhysicalTable,
        parameter_name: &str,
        parameter_value: &Value<String>,
    ) -> (&'a Column<'a>, Ordering) {
        let parameter = match &self.kind {
            OrderByParameterTypeKind::Composite { parameters } => {
                parameters.iter().find(|p| p.name == parameter_name)
            }
            _ => None,
        };

        let column = table.get_column(&parameter.unwrap().name()).unwrap();

        (column, Self::ordering(parameter_value))
    }

    fn ordering<'a>(argument: &Value<String>) -> Ordering {
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
