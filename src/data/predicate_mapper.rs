use std::collections::HashMap;

use crate::sql::table::PhysicalTable;
use crate::sql::{column::Column, predicate::Predicate};

use crate::model::{
    predicate::*,
    system::ModelSystem,
};

use graphql_parser::schema::Value;

impl PredicateParameter {
    pub fn predicate<'a>(
        &self,
        argument_value: &'a ArgumentColumn<'_>,
        table: &'a PhysicalTable,
        system: &'a ModelSystem,
    ) -> Predicate<'a> {
        let parameter_type = system
            .parameter_types
            .find_predicate_parameter_type(&self.type_name)
            .unwrap();

        match &parameter_type.kind {
            PredicateParameterTypeKind::Primitive => {
                let argument_column = match argument_value {
                    ArgumentColumn::Primitive(value) => value,
                    _ => todo!(),
                };
                Predicate::Eq(table.get_column(&self.name).unwrap(), &argument_column)
            }
            PredicateParameterTypeKind::Composite { parameters } => {
                parameters.iter().fold(Predicate::True, |acc, parameter| {
                    let new_argument_value = match argument_value {
                        ArgumentColumn::Object(value) => value.get(&parameter.name),
                        ArgumentColumn::Primitive(_) => todo!(),
                    }.unwrap();
                    let new_predicate = parameter.predicate(new_argument_value, table, system);
                    Predicate::And(Box::new(acc), Box::new(new_predicate))
                })
            }
        }
    }
}

pub struct ArgumentSupplier<'a> {
    argument_name: String,
    pub argument_value: ArgumentColumn<'a>,
}

pub enum ArgumentColumn<'a> {
    Primitive(Column<'a>),
    Object(HashMap<String, ArgumentColumn<'a>>),
}

impl<'a> ArgumentSupplier<'a> {
    pub fn new(argument_name: String, argument_value: Value<String>) -> Self {
        Self {
            argument_name: argument_name,
            argument_value: Self::param_value(argument_value),
        }
    }

    fn param_value(value: Value<String>) -> ArgumentColumn<'a> {
        match value {
            Value::Variable(_) => todo!(),
            Value::Int(v) => {
                // TODO: Work with the database schema to cast to appropriate i32, etc type
                ArgumentColumn::Primitive(Column::Literal(Box::new(v.as_i64().unwrap() as i32)))
            }
            Value::Float(v) => ArgumentColumn::Primitive(Column::Literal(Box::new(v))),
            Value::String(v) => ArgumentColumn::Primitive(Column::Literal(Box::new(v.to_owned()))),
            Value::Boolean(v) => ArgumentColumn::Primitive(Column::Literal(Box::new(v))),
            Value::Null => todo!(),
            Value::Enum(v) => ArgumentColumn::Primitive(Column::Literal(Box::new(v.to_owned()))), // We might need guidance from database to do a correct translation
            Value::List(_) => todo!(),
            Value::Object(elems) => {
                let mapped: HashMap<_, _> = elems
                    .iter()
                    .map(|elem| (elem.0.to_owned(), Self::param_value(elem.1.to_owned())))
                    .collect();
                ArgumentColumn::Object(mapped)
            }
        }
    }
}
