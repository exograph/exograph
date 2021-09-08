use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use postgres::{
    types::{FromSqlOwned, ToSql},
    Client, GenericClient, Row,
};

use crate::sql::ExpressionContext;

use super::{
    sql_operation::TemplateSQLOperation, OperationExpression, SQLOperation, SQLParam, SQLValue,
};

#[derive(Debug)]
pub enum TransactionScript<'a> {
    Single(TransactionStep<'a>),
    Multi(Vec<Rc<TransactionStep<'a>>>, TransactionStep<'a>),
}

impl<'a> TransactionScript<'a> {
    pub fn execute<T: FromSqlOwned>(
        &'a self,
        client: &mut Client,
        extractor: fn(Row) -> Result<T>,
    ) -> Result<Vec<T>> {
        match self {
            Self::Single(step) => step.execute_and_extract(client, extractor),
            Self::Multi(init, last) => {
                println!("Starting transaction");
                let mut tx = client.transaction()?;
                for step in init {
                    step.execute(&mut tx)?;
                }

                let result = last.execute_and_extract(&mut tx, extractor);
                println!("Committing transaction");
                result
            }
        }
    }
}

#[derive(Debug)]
pub enum TransactionStep<'a> {
    Concrete(ConcreteTransactionStep<'a>),
    Template(TemplateTransactionStep<'a>),
}

impl<'a> TransactionStep<'a> {
    pub fn execute(&self, client: &mut impl GenericClient) -> Result<()> {
        match self {
            Self::Concrete(step) => step.execute(client),
            Self::Template(step) => {
                let concrete = step.resolve();

                let mut result = Ok(());
                for substep in concrete {
                    result = substep.execute(client);
                    step.values.borrow_mut().extend(substep.values.into_inner());
                }
                result
            }
        }
    }

    fn execute_and_extract<T: FromSqlOwned>(
        &self,
        client: &mut impl GenericClient,
        extractor: fn(Row) -> Result<T>,
    ) -> Result<Vec<T>> {
        match self {
            Self::Concrete(step) => step.execute_and_extract(client, extractor),
            Self::Template(step) => {
                let concrete = step.resolve();

                match concrete.as_slice() {
                    [init @ .., last] => {
                        for substep in init {
                            substep.execute(client)?;
                        }
                        last.execute_and_extract(client, extractor)
                    }
                    _ => Ok(vec![]),
                }
            }
        }
    }

    pub fn resolved_value(&'a self) -> &'a RefCell<Vec<Vec<SQLValue>>> {
        match self {
            Self::Concrete(step) => &step.values,
            Self::Template(step) => &step.values,
        }
    }

    pub fn get_value(&'a self, row_index: usize, col_index: usize) -> &'a (dyn SQLParam + 'static) {
        match self {
            Self::Concrete(step) => step.get_value(row_index, col_index),
            Self::Template(step) => step.get_value(row_index, col_index),
        }
    }
}

#[derive(Debug)]
pub struct ConcreteTransactionStep<'a> {
    pub operation: SQLOperation<'a>,
    pub values: RefCell<Vec<Vec<SQLValue>>>,
}

impl<'a> ConcreteTransactionStep<'a> {
    pub fn new(operation: SQLOperation<'a>) -> Self {
        Self {
            operation,
            values: RefCell::new(vec![]),
        }
    }

    pub fn execute(&'a self, client: &mut impl GenericClient) -> Result<()> {
        let rows = self.run_query(client)?;

        let values = Self::result_extractor(&rows)?;
        *self.values.borrow_mut() = values;

        Ok(())
    }

    fn execute_and_extract<T: FromSqlOwned>(
        &'a self,
        client: &mut impl GenericClient,
        extractor: fn(Row) -> Result<T>,
    ) -> Result<Vec<T>> {
        let rows = self.run_query(client)?;

        rows.into_iter().map(extractor).collect()
    }

    fn run_query(&'a self, client: &mut impl GenericClient) -> Result<Vec<Row>> {
        let sql_operation = &self.operation;
        let mut context = ExpressionContext::default();
        let binding = sql_operation.binding(&mut context);

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        println!("Executing transaction step: {}", binding.stmt);
        client
            .query(binding.stmt.as_str(), &params[..])
            .context("PostgreSQL query failed")
    }

    fn result_extractor(rows: &[Row]) -> Result<Vec<Vec<SQLValue>>> {
        Ok(rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(move |col_index| row.get::<usize, SQLValue>(col_index))
                    .collect::<Vec<SQLValue>>()
            })
            .collect::<Vec<_>>())
    }

    pub fn get_value(&self, row_index: usize, col_index: usize) -> &'a (dyn SQLParam + 'static) {
        let reference = &self.values.borrow()[row_index][col_index];

        // SAFETY: This is safe because we are casting an SQLValue to a dyn SQLParam and we know that we keep
        // around a reference to the original SQLValue as long as `self` is alive.
        // Ideally, we shouldn't need unsafe here (see https://github.com/payalabs/payas/issues/176)
        unsafe {
            let ptr: *const std::ffi::c_void = std::mem::transmute(reference);
            let sql_param: &'a SQLValue = &*(ptr as *const SQLValue);

            sql_param
        }
    }
}

type TransactionStepResult = Vec<SQLValue>;

#[derive(Debug)]
pub struct TemplateTransactionStep<'a> {
    pub operation: TemplateSQLOperation<'a>,
    pub step: Rc<TransactionStep<'a>>,
    pub values: RefCell<Vec<Vec<SQLValue>>>,
}

impl<'a> TemplateTransactionStep<'a> {
    pub fn resolve(&'a self) -> Vec<ConcreteTransactionStep<'a>> {
        self.operation
            .resolve(self.step.clone())
            .into_iter()
            .map(|operation| ConcreteTransactionStep {
                operation,
                values: RefCell::new(vec![]),
            })
            .collect()
    }

    // TODO: Dedup from ConcreteTransactionStep
    pub fn get_value(&self, row_index: usize, col_index: usize) -> &'a (dyn SQLParam + 'static) {
        let reference = &self.values.borrow()[row_index][col_index];

        unsafe {
            let ptr: *const std::ffi::c_void = std::mem::transmute(reference);
            let sql_param: &'a SQLValue = &*(ptr as *const SQLValue);

            sql_param
        }
    }
}
