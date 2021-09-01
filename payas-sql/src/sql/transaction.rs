use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use postgres::{
    types::{FromSqlOwned, ToSql},
    Client, Row,
};

use crate::sql::ExpressionContext;

use super::{OperationExpression, SQLOperation, SQLParam, SQLValue};

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
                for step in init {
                    step.execute(client)?;
                }

                last.execute_and_extract(client, extractor)
            }
        }
    }
}

#[derive(Debug)]
pub struct TransactionStep<'a> {
    pub operation: SQLOperation<'a>,
    pub values: RefCell<Vec<Vec<SQLValue>>>,
}

impl<'a> TransactionStep<'a> {
    pub fn new(operation: SQLOperation<'a>) -> Self {
        Self {
            operation,
            values: RefCell::new(vec![]),
        }
    }

    pub fn execute(&'a self, client: &mut Client) -> Result<()> {
        let rows = self.run_query(client)?;

        let values = Self::result_extractor(&rows)?;
        *self.values.borrow_mut() = values;

        Ok(())
    }

    fn execute_and_extract<T: FromSqlOwned>(
        &'a self,
        client: &mut Client,
        extractor: fn(Row) -> Result<T>,
    ) -> Result<Vec<T>> {
        let rows = self.run_query(client)?;

        rows.into_iter().map(extractor).collect()
    }

    fn run_query(&'a self, client: &mut Client) -> Result<Vec<Row>> {
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

        unsafe {
            let ptr: *const std::ffi::c_void = std::mem::transmute(reference);
            let sql_param: &'a SQLValue = &*(ptr as *const SQLValue);

            sql_param
        }
    }
}

type TransactionStepResult = Vec<SQLValue>;

// TODO: re-enable after https://github.com/payalabs/payas/issues/175

//#[cfg(test)]
//mod tests {
// //}
