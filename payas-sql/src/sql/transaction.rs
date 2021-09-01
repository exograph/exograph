use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use postgres::{
    types::{FromSqlOwned, ToSql},
    Client, Row,
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
                for step in init {
                    step.execute(client)?;
                }

                last.execute_and_extract(client, extractor)
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
    pub fn execute(&self, client: &mut Client) -> Result<()> {
        match self {
            Self::Concrete(step) => step.execute(client),
            Self::Template(step) => {
                let concrete = step.resolve();
                let result = concrete.execute(client);
                *step.values.borrow_mut() = concrete.values.into_inner();
                result
            }
        }
    }

    fn execute_and_extract<T: FromSqlOwned>(
        &self,
        client: &mut Client,
        extractor: fn(Row) -> Result<T>,
    ) -> Result<Vec<T>> {
        match self {
            Self::Concrete(step) => step.execute_and_extract(client, extractor),
            Self::Template(step) => {
                let concrete = step.resolve();
                concrete.execute_and_extract(client, extractor)
            }
        }
    }

    pub fn resolved_value(&'a self) -> &'a RefCell<Vec<Vec<SQLValue>>> {
        match self {
            Self::Concrete(step) => &step.values,
            Self::Template(step) => &step.values,
        }
    }

    pub fn get_value(&self, row_index: usize, col_index: usize) -> &'a (dyn SQLParam + 'static) {
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

#[derive(Debug)]
pub struct TemplateTransactionStep<'a> {
    pub operation: TemplateSQLOperation<'a>,
    pub step: &'a TransactionStep<'a>,
    pub values: RefCell<Vec<Vec<SQLValue>>>,
}

impl<'a> TemplateTransactionStep<'a> {
    pub fn resolve(&self) -> ConcreteTransactionStep<'a> {
        ConcreteTransactionStep {
            operation: self.operation.resolve(&self.step),
            values: RefCell::new(vec![]),
        }
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

// TODO: re-enable after https://github.com/payalabs/payas/issues/175

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    use crate::sql::{
        column::{Column, IntBits, PhysicalColumn, PhysicalColumnType, ProxyColumn},
        insert::TemplateInsert,
        predicate::Predicate,
        transaction::{ConcreteTransactionStep, TransactionStep},
        PhysicalTable, SQLOperation,
    };
    use anyhow::{bail, Context, Result};
    use postgres::NoTls;
    use postgres::{Client, Config};

    type ConnectionString = String;
    type DbUsername = String;

    pub fn get_client(url: &str) -> Result<Client> {
        // TODO validate dbname

        // parse connection string
        let mut config = url
            .parse::<Config>()
            .context("Failed to parse PostgreSQL connection string")?;

        // "The postgres database is a default database meant for use by users, utilities and third party applications."
        config.dbname("payas-test");

        // run creation query
        let client: Client = config.connect(NoTls)?;

        // return
        Ok(client)
    }

    /// Connect to the specified PostgreSQL database and attempt to run a query.
    pub fn run_psql(query: &str, url: &str) -> Result<()> {
        let mut client = url.parse::<Config>()?.connect(NoTls)?;
        client
            .simple_query(query)
            .context(format!("PostgreSQL query failed: {}", query))
            .map(|_| ())
    }

    /// Drop the specified database at the specified PostgreSQL server and
    /// return on success.
    pub fn dropdb_psql(dbname: &str, url: &str) -> Result<()> {
        let mut config = url.parse::<Config>()?;

        // "The postgres database is a default database meant for use by users, utilities and third party applications."
        config.dbname("postgres");

        let mut client = config.connect(NoTls)?;

        let query: String = format!("DROP DATABASE \"{}\"", dbname);
        client
            .execute(query.as_str(), &[])
            .context("PostgreSQL drop database query failed")
            .map(|_| ())
    }

    pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
        match row.try_get(0) {
            Ok(col) => Ok(col),
            Err(err) => bail!("Got row without any columns {}", err),
        }
    }

    //    #[test]
    //    fn basic_transaction_step_test() {
    //        let connection_string = "postgresql://noneucat:noneucat@localhost:5432/payas";
    //
    //        let mut client = get_client(connection_string).unwrap();
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
    //            columns: vec![dst_age_col.clone()],
    //        };
    //
    //        let src_name_phys_col = src_table.get_physical_column("name").unwrap();
    //        let src_age_phys_col = src_table.get_physical_column("age").unwrap();
    //        let src_age_col = src_table.get_column("age").unwrap();
    //
    //        let name_literal = Column::Literal(Box::new("abc"));
    //        let age_literal = Column::Literal(Box::new(18i16));
    //        let insertion_op = src_table.insert(
    //            vec![src_name_phys_col, src_age_phys_col],
    //            vec![vec![&name_literal, &age_literal]],
    //            vec![&src_age_col],
    //        );
    //        let step_a = Rc::new(TransactionStep {
    //            operation: SQLOperation::Insert(insertion_op),
    //            values: RefCell::new(vec![]),
    //        });
    //
    //        let lazy_col = Column::Lazy {
    //            row_index: 0,
    //            col_index: 0,
    //            step: &step_a,
    //        };
    //        let insertion_op = dst_table.insert(vec![&dst_age_col], vec![vec![&lazy_col]], vec![]);
    //
    //        let step_b = TransactionStep {
    //            operation: SQLOperation::Insert(insertion_op),
    //            values: RefCell::new(vec![]),
    //        };
    //
    //        let transaction_script = TransactionScript::Multi(vec![step_a.clone()], step_b);
    //
    //        let e = transaction_script.execute::<String>(&mut client, extractor);
    //        println!("{:?}", e)
    //    }

    // CREATE TABLE people (
    //     name TEXT,
    //     age SMALLINT
    // );

    // CREATE TABLE ages (
    //     age SMALLINT
    // );

    // #[test]
    // fn template_transaction_step_test() {
    //     let people_table = PhysicalTable {
    //         name: "people".to_string(),
    //         columns: vec![
    //             PhysicalColumn {
    //                 table_name: "people".to_string(),
    //                 column_name: "name".to_string(),
    //                 typ: PhysicalColumnType::String { length: None },
    //                 is_pk: false,
    //                 is_autoincrement: false,
    //             },
    //             PhysicalColumn {
    //                 table_name: "people".to_string(),
    //                 column_name: "age".to_string(),
    //                 typ: PhysicalColumnType::Int { bits: IntBits::_16 },
    //                 is_pk: false,
    //                 is_autoincrement: false,
    //             },
    //         ],
    //     };

    //     let ages_table = PhysicalTable {
    //         name: "ages".to_string(),
    //         columns: vec![PhysicalColumn {
    //             table_name: "ages".to_string(),
    //             column_name: "age".to_string(),
    //             typ: PhysicalColumnType::Int { bits: IntBits::_16 },
    //             is_pk: false,
    //             is_autoincrement: false,
    //         }],
    //     };

    //     let people_name_phys_col = people_table.get_physical_column("name").unwrap();
    //     let people_age_col = people_table.get_column("age").unwrap();

    //     let name_literal = Column::Literal(Box::new("abc"));
    //     let update_op = people_table.update(
    //         vec![(people_name_phys_col, &name_literal)],
    //         &Predicate::True,
    //         vec![&people_age_col],
    //     );

    //     let step_a = Rc::new(TransactionStep::Concrete(ConcreteTransactionStep {
    //         operation: SQLOperation::Update(update_op),
    //         values: RefCell::new(vec![]),
    //     }));

    //     let ages_age_phys_col = ages_table.get_physical_column("age").unwrap();
    //     let ages_age_col = ages_table.get_column("age").unwrap();
    //     let age_proxy_column = ProxyColumn::Template {
    //         col_index: 0,
    //         step: &step_a,
    //     };

    //     let insert_op_template = TemplateSQLOperation::Insert(TemplateInsert {
    //         table: &ages_table,
    //         column_names: vec![ages_age_phys_col],
    //         column_values_seq: vec![&age_proxy_column],
    //         returning: vec![&ages_age_col],
    //     });
    //     let step_b = TransactionStep::Template(TemplateTransactionStep {
    //         operation: insert_op_template,
    //         step: &step_a,
    //         values: RefCell::new(vec![]),
    //     });

    //     let transaction_script = TransactionScript::Multi(vec![step_a.clone()], step_b);

    //     let connection_string = "postgresql://ramnivas@localhost:5432/payas-test";

    //     let mut client = get_client(connection_string).unwrap();
    //     let e = transaction_script.execute::<i16>(&mut client, extractor);
    //     println!("{:?}", e)
    // }
}
