use std::collections::HashMap;

use crate::model::types::*;
use crate::sql::ExpressionContext;
use crate::sql::{column::Column, predicate::Predicate};
use crate::sql::{table::PhysicalTable, Expression};
use graphql_parser::{
    query::{Field, Selection},
    schema::Value,
};

use crate::execution::resolver::OutputName;
use crate::{
    execution::query_context::QueryResponse,
    model::{system::ModelSystem, types::Operation},
    sql::database::Database,
};

pub struct DataContext<'a> {
    pub system: ModelSystem,
    pub database: Database<'a>,
}

impl<'a> DataContext<'a> {
    pub fn resolve(&self, field: &Field<'_, String>) -> QueryResponse {
        let operation = self.system.queries.iter().find(|q| q.name == field.name);
        operation.unwrap().resolve(field, self)
    }
}

impl Operation {
    fn resolve(&self, field: &Field<'_, String>, data_context: &DataContext) -> QueryResponse {
        let string_response = self.operation(field, data_context);
        QueryResponse::Raw(string_response)
    }

    fn operation(&self, field: &Field<'_, String>, data_context: &DataContext) -> String {
        let table = self.physical_table(data_context).unwrap();
        let table_name = &table.name;

        let predicate_arguments = &field.arguments.iter().fold(None, |_acc, argument| {
            let (argument_name, argument_value) = argument;

            let parameter = self.parameters.iter().find(|p| &p.name == argument_name);

            match parameter.map(|p| &p.role) {
                Some(ParameterRole::Predicate) => Some((argument_name, argument_value)),
                Some(ParameterRole::OrderBy) => todo!(),
                Some(ParameterRole::Data) => todo!(),
                None => None,
            }
        });

        let argument_supplier =
            predicate_arguments.map(|ps| ArgumentSupplier::new(ps.0.to_owned(), ps.1.to_owned()));
        let predicate = argument_supplier.as_ref().map(|ref argument_supplier| self.predicate(argument_supplier, table, &data_context.system));
        let content_object = self.content_select(field, table_name);

        let agg_column = Column::JsonAgg(&content_object);
        let single_column = vec![&content_object];
        let vector_column = vec![&agg_column];
        let single_select = table.select(&single_column, predicate.as_ref());
        let vector_select = table.select(&vector_column, predicate.as_ref());

        let mut expression_context = ExpressionContext::new();

        let binding = match self.return_type.type_modifier {
            ModelTypeModifier::Optional => single_select.binding(&mut expression_context),
            ModelTypeModifier::NonNull => single_select.binding(&mut expression_context),
            ModelTypeModifier::List => vector_select.binding(&mut expression_context),
        };

        data_context.database.execute(binding)
    }

    fn content_select(&self, field: &Field<'_, String>, table_name: &str) -> Column {
        let column_specs: Vec<_> = field
            .selection_set
            .items
            .iter()
            .map(|item| match item {
                Selection::Field(field) => (
                    field.output_name(),
                    Column::Physical {
                        table_name: table_name.to_string(),
                        column_name: field.name.clone(),
                    },
                ),
                _ => todo!(),
            })
            .collect();

        Column::JsonObject(column_specs)
    }

    fn physical_table<'a>(&self, data_context: &'a DataContext) -> Option<&'a PhysicalTable<'a>> {
        data_context
            .system
            .find_type(&self.return_type.type_name)
            .map(|t| match &t.kind {
                ModelTypeKind::Composite {
                    model_fields: _,
                    table_name,
                } => Some(table_name),
                _ => None,
            })
            .flatten()
            .map(|table_name| data_context.database.get_table(table_name))
            .flatten()
    }

    fn predicate<'a>(
        &self,
        argument_supplier: &'a ArgumentSupplier<'_>,
        table: &'a PhysicalTable,
        system: &'a ModelSystem,
    ) -> Predicate<'a> {
        let ArgumentSupplier {
            argument_name,
            argument_value,
        } = argument_supplier;
        let parameter = self
            .parameters
            .iter()
            .find(|p| &p.name == argument_name)
            .unwrap();

        let parameter_type = system
            .parameter_types
            .find_parameter_type(&parameter.type_name)
            .unwrap();

        // TODO: Make this generic and aware of other parameters (general filter, order by, etc)
        match &parameter_type.kind {
            ParameterTypeKind::Primitive => {
                let argument_column = match argument_value {
                    ArgumentColumn::Primitive(value) => value,
                    _ => todo!(),
                };
                Predicate::Eq(table.get_column(&parameter.name).unwrap(), &argument_column)
            }
            ParameterTypeKind::Composite { parameters: _ } => {
                // predicates.fold(Predicate::True, |acc, predicate| {
                //     Predicate::And(Box::new(acc), Box::new(predicate))
                // })
                todo!()
            }
            ParameterTypeKind::Enum { values: _ } => todo!(),
        }
    }
}

struct ArgumentSupplier<'a> {
    argument_name: String,
    argument_value: ArgumentColumn<'a>,
}

enum ArgumentColumn<'a> {
    Primitive(Column<'a>),
    Object(HashMap<String, ArgumentColumn<'a>>),
}

impl<'a> ArgumentSupplier<'a> {
    fn new(argument_name: String, argument_value: Value<String>) -> Self {
        Self {
            argument_name: argument_name,
            argument_value: Self::param_value(argument_value),
        }
    }

    fn param_value(value: Value<String>) -> ArgumentColumn<'a> {
        match value {
            Value::Variable(_) => todo!(),
            Value::Int(v) => {
                // TODO: Unhack this (we can't access the underlying value of Number since it is declared pub(crate)))
                let v_string = format!("{:?}", v);
                let number_only = &v_string[..v_string.len() - 1][7..]; // Remove the Number(...) shell
                let v_num: i32 = number_only.parse().unwrap(); // TODO: Work with the database schema to cast to appropriate i32, etc type
                ArgumentColumn::Primitive(Column::Literal(Box::new(v_num)))
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
