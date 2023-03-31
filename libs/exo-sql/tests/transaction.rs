mod common;
/*
use std::{cell::RefCell, rc::Rc};

use anyhow::{bail, Result};
use exo_sql::sql::column::PhysicalColumn;
use exo_sql::sql::column::ProxyColumn;
use exo_sql::sql::database::Database;
use exo_sql::sql::predicate::Predicate;
use exo_sql::sql::transaction::*;
use exo_sql::sql::PhysicalTable;
use exo_sql::sql::TemplateInsert;
use exo_sql::sql::TemplateSQLOperation;
use exo_sql::sql::{column::Column, SQLOperation};
use tokio_postgres::{types::FromSqlOwned, Row};

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}

struct PeopleTableInfo<'a> {
    pub table: &'a PhysicalTable,
    pub name_phys_col: &'a PhysicalColumn,
    pub age_phys_col: &'a PhysicalColumn,
    pub name_col: &'a Column<'a>,
    pub age_col: &'a Column<'a>,
}

struct AgesTableInfo<'a> {
    pub table: &'a PhysicalTable,
    pub age_phys_col: &'a PhysicalColumn,
    pub age_col: &'a Column<'a>,
}
*/
/*
fn with_setup(test_name: &str, test_fn: impl FnOnce(&Database, &PeopleTableInfo, &AgesTableInfo)) {
    let ctx = common::create_context(test_name).unwrap();
    let db = ctx.test_db.as_ref().unwrap();

    /////
    // create our tables in database
    /////

    let people_table = common::create_physical_table(
        db,
        "people",
        r##"
        CREATE TABLE people (
            name TEXT,
            age SMALLINT
        );
    "##,
    );

    let ages_table = common::create_physical_table(
        db,
        "ages",
        r##"
        CREATE TABLE ages (
            age SMALLINT
        );
    "##,
    );

    let people_name_phys_col = people_table.get_physical_column("name").unwrap();
    let people_age_phys_col = people_table.get_physical_column("age").unwrap();
    let people_name_col = people_table.get_column("name").unwrap();
    let people_age_col = people_table.get_column("age").unwrap();

    let ages_age_col = ages_table.get_column("age").unwrap();
    let ages_age_phys_col = ages_table.get_physical_column("age").unwrap();

    let people_table_info = PeopleTableInfo {
        table: &people_table,
        name_phys_col: people_name_phys_col,
        age_phys_col: people_age_phys_col,
        name_col: &people_name_col,
        age_col: &people_age_col,
    };

    let ages_table_info = AgesTableInfo {
        table: &ages_table,
        age_phys_col: ages_age_phys_col,
        age_col: &ages_age_col,
    };

    test_fn(db, &people_table_info, &ages_table_info);
}
*/
/*
#[test]
/// Minimal example and test of a pair of TransactionSteps in a
/// TransactionScript.
fn basic_transaction() {
    with_setup(
        "basic_transaction_step_test",
        |db, people_table_info, ages_table_info| {
            // name  | age
            // ------------
            // teen  | 16
            // adult | 20

            let teen_name = Column::Literal(Box::new("teen"));
            let teen_age = Column::Literal(Box::new(16i16));
            let adult_name = Column::Literal(Box::new("adult"));
            let adult_age = Column::Literal(Box::new(20i16));

            let insertion_op = people_table_info.table.insert(
                vec![
                    people_table_info.name_phys_col,
                    people_table_info.age_phys_col,
                ],
                vec![vec![&teen_name, &teen_age], vec![&adult_name, &adult_age]],
                vec![people_table_info.age_col.into()],
            );

            let step_a = Rc::new(TransactionStep::Concrete(ConcreteTransactionStep {
                operation: SQLOperation::Insert(insertion_op),
                values: RefCell::new(vec![]),
            }));

            // insertion from people_table into ages_table
            let lazy_col = ProxyColumn::Template {
                col_index: 0,
                step: step_a.clone(),
            };

            let insertion_op = TemplateInsert {
                table: ages_table_info.table,
                column_names: vec![ages_table_info.age_phys_col],
                column_values_seq: vec![vec![lazy_col]],
                returning: vec![ages_table_info.age_col.into()],
            };

            let step_b = TransactionStep::Template(TemplateTransactionStep {
                operation: TemplateSQLOperation::Insert(insertion_op),
                step: step_a.clone(),
                values: RefCell::new(vec![]),
            });

            let transaction_script = TransactionScript::Multi(vec![step_a.clone()], step_b);

            let result = transaction_script
                .execute::<i16>(&mut db.get_client().unwrap(), extractor)
                .unwrap();

            assert_eq!(result, vec![16i16, 20i16]);

            // Check that the transaction did commit by executing a standalone query after running the transaction script
            let selected_ages: Vec<_> = db
                .get_client()
                .unwrap()
                .query("SELECT age FROM ages", &[])
                .unwrap()
                .iter()
                .map(|row| {
                    let age: i16 = row.get("age");
                    age
                })
                .collect();
            assert_eq!(selected_ages, vec![16i16, 20i16]);
        },
    );
}

#[test]
fn transaction_zero_matches() {
    with_setup(
        "transaction_zero_matches",
        |db, people_table_info, ages_table_info| {
            let name_literal = Column::Literal(Box::new("abc"));
            let update_op = people_table_info.table.update(
                vec![(people_table_info.name_phys_col, &name_literal)],
                Predicate::True.into(),
                vec![people_table_info.age_col.into()],
            );

            let step_a = Rc::new(TransactionStep::Concrete(ConcreteTransactionStep {
                operation: SQLOperation::Update(update_op),
                values: RefCell::new(vec![]),
            }));

            let age_proxy_column = ProxyColumn::Template {
                col_index: 0,
                step: step_a.clone(),
            };

            let insert_op_template = TemplateSQLOperation::Insert(TemplateInsert {
                table: ages_table_info.table,
                column_names: vec![ages_table_info.age_phys_col],
                column_values_seq: vec![vec![age_proxy_column]],
                returning: vec![ages_table_info.age_col.into()],
            });
            let step_b = TransactionStep::Template(TemplateTransactionStep {
                operation: insert_op_template,
                step: step_a.clone(),
                values: RefCell::new(vec![]),
            });

            let transaction_script = TransactionScript::Multi(vec![step_a.clone()], step_b);

            // Since update will affect zero rows, insert will affect zero rows.
            let result = transaction_script
                .execute::<i16>(&mut db.get_client().unwrap(), extractor)
                .unwrap();

            assert_eq!(result, Vec::<i16>::new());
        },
    );
}
*/
