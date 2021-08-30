use std::cell::RefCell;

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
    Multi(Vec<TransactionStep<'a>>, TransactionStep<'a>),
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
//    use crate::sql::{PhysicalTable, column::{Column, IntBits, PhysicalColumn, PhysicalColumnType}, select::Select};
//    use anyhow::{Context, Result};
//    use postgres::NoTls;
//    use postgres::{Client, Config};
//
//    type ConnectionString = String;
//    type DbUsername = String;
//
//    pub fn get_client(url: &str) -> Result<Client> {
//        // TODO validate dbname
//
//        // parse connection string
//        let mut config = url
//            .parse::<Config>()
//            .context("Failed to parse PostgreSQL connection string")?;
//
//        // "The postgres database is a default database meant for use by users, utilities and third party applications."
//        config.dbname("payas");
//
//        // run creation query
//        let mut client: Client = config.connect(NoTls)?;
//
//        // return
//        Ok(client)
//    }
//
//    /// Connect to the specified PostgreSQL database and attempt to run a query.
//    pub fn run_psql(query: &str, url: &str) -> Result<()> {
//        let mut client = url.parse::<Config>()?.connect(NoTls)?;
//        client
//            .simple_query(query)
//            .context(format!("PostgreSQL query failed: {}", query))
//            .map(|_| ())
//    }
//
//    /// Drop the specified database at the specified PostgreSQL server and
//    /// return on success.
//    pub fn dropdb_psql(dbname: &str, url: &str) -> Result<()> {
//        let mut config = url.parse::<Config>()?;
//
//        // "The postgres database is a default database meant for use by users, utilities and third party applications."
//        config.dbname("postgres");
//
//        let mut client = config.connect(NoTls)?;
//
//        let query: String = format!("DROP DATABASE \"{}\"", dbname);
//        client
//            .execute(query.as_str(), &[])
//            .context("PostgreSQL drop database query failed")
//            .map(|_| ())
//    }
//
//    use super::*;
//    #[test]
//    fn basic_transaction_step_test() {
//        let connection_string = "postgresql://noneucat:noneucat@localhost:5432/payas";
//        ///
//
//        let mut client = get_client( connection_string).unwrap();
//
//        let src_table = PhysicalTable {
//            name: "people".to_string(),
//            columns: vec![
//                PhysicalColumn {
//                    table_name: "people".to_string(),
//                    column_name: "name".to_string(),
//                    typ: PhysicalColumnType::String { length: None },
//                    is_pk: false,
//                    is_autoincrement: false,
//                },
//                PhysicalColumn {
//                    table_name: "people".to_string(),
//                    column_name: "age".to_string(),
//                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
//                    is_pk: false,
//                    is_autoincrement: false,
//                },
//            ],
//        };
//
//        let dst_age_col = PhysicalColumn {
//            table_name: "ages".to_string(),
//            column_name: "age".to_string(),
//            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
//            is_pk: false,
//            is_autoincrement: false,
//        };
//
//        let dst_table = PhysicalTable {
//            name: "ages".to_string(),
//            columns: vec![
//                dst_age_col.clone()
//            ],
//        };
//
//        let src_name_phys_col = src_table.get_physical_column("name").unwrap();
//        let src_age_phys_col = src_table.get_physical_column("age").unwrap();
//        let src_age_col = src_table.get_column("age").unwrap();
//
//        let name_literal = Column::Literal(Box::new("abc"));
//        let age_literal = Column::Literal(Box::new(18i16));
//        let insertion_op = src_table.insert(
//            vec![
//                src_name_phys_col,
//                src_age_phys_col
//            ],
//            vec![
//                vec![
//                    &name_literal,
//                    &age_literal
//                ]
//            ],
//            vec![&src_age_col]
//        );
//        let step_a = TransactionStep {
//            operation: SQLOperation::Insert(insertion_op),
//            values: RefCell::new(vec![]),
//        };
//
//        let lazy_col = Column::Lazy {
//                        row_index: 0, col_index: 0,
//                        step: &step_a,
//                    };
//        let insertion_op = dst_table.insert(
//            vec![&dst_age_col],
//            vec![
//                vec![
//                    &lazy_col
//                ]
//            ],
//            vec![]
//        );
//
//        let step_b = TransactionStep {
//            operation: SQLOperation::Insert(insertion_op),
//            values: RefCell::new(vec![]),
//        };
//
//        let mut transaction_script = TransactionScriptX::Multi(
//            vec![&step_a], step_b
//        );
//
//        let e = transaction_script.execute(&mut client);
//        println!("{:?}", e)
//    }
//}
