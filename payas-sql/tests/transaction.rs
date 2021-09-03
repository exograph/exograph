mod common;

use std::{cell::RefCell, rc::Rc};

use anyhow::{bail, Result};
use payas_sql::sql::transaction::*;
use payas_sql::sql::{column::Column, SQLOperation};
use postgres::{types::FromSqlOwned, Row};

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}

#[test]
/// Minimal example and test of a pair of TransactionSteps in a
/// TransactionScript.
fn basic_transaction_step_test() {
    let ctx = common::create_context("transaction_step").unwrap();
    let db = ctx.test_db.as_ref().unwrap();

    /////
    // create our tables in database
    /////

    let src_table = common::create_physical_table(
        db,
        "people",
        r##"
        CREATE TABLE people (
            name TEXT,
            age SMALLINT
        );
    "##,
    );

    let dst_table = common::create_physical_table(
        db,
        "ages",
        r##"
        CREATE TABLE ages (
            age SMALLINT
        );
    "##,
    );

    let src_name_phys_col = src_table.get_physical_column("name").unwrap();
    let src_age_phys_col = src_table.get_physical_column("age").unwrap();
    let src_age_col = src_table.get_column("age").unwrap();

    let dst_age_col = dst_table.get_column("age").unwrap();
    let dst_age_phys_col = dst_table.get_physical_column("age").unwrap();

    /////
    // begin constructing our transaction steps
    /////

    // initialization of src_table
    //
    // name | age
    // ----------
    // abc  | 18
    //
    let name_literal = Column::Literal(Box::new("abc"));
    let age_literal = Column::Literal(Box::new(18i16));
    let insertion_op = src_table.insert(
        vec![src_name_phys_col, src_age_phys_col],
        vec![vec![&name_literal, &age_literal]],
        vec![&src_age_col],
    );

    let step_a = Rc::new(TransactionStep {
        operation: SQLOperation::Insert(insertion_op),
        values: RefCell::new(vec![]),
    });

    // insertion from src_table into dst_table
    let lazy_col = Column::Lazy {
        row_index: 0,
        col_index: 0,
        step: &step_a,
    };

    let insertion_op = dst_table.insert(
        vec![&dst_age_phys_col],
        vec![vec![&lazy_col]],
        vec![&dst_age_col],
    );

    let step_b = TransactionStep {
        operation: SQLOperation::Insert(insertion_op),
        values: RefCell::new(vec![]),
    };

    /////
    // construct and run our TransactionScript
    /////

    let transaction_script = TransactionScript::Multi(vec![step_a.clone()], step_b);

    let result = transaction_script
        .execute::<i16>(&mut db.get_client().unwrap(), extractor)
        .unwrap();

    assert!(age_literal.get_value().eq(&result[0]))
}
