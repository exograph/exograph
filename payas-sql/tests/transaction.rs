mod common;

use std::{cell::RefCell, rc::Rc};

use anyhow::{bail, Result};
use payas_sql::sql::transaction::*;
use payas_sql::sql::{
    column::{Column, IntBits, PhysicalColumn, PhysicalColumnType},
    PhysicalTable, SQLOperation,
};
use postgres::{types::FromSqlOwned, Row};

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}

#[test]
fn basic_transaction_step_test() {
    let mut ctx = common::create_database("transaction_step").unwrap();
    let mut client = ctx.get_database().get_client().unwrap();

    let src_table = PhysicalTable {
        name: "people".to_string(),
        columns: vec![
            PhysicalColumn {
                table_name: "people".to_string(),
                column_name: "name".to_string(),
                typ: PhysicalColumnType::String { length: None },
                is_pk: false,
                is_autoincrement: false,
            },
            PhysicalColumn {
                table_name: "people".to_string(),
                column_name: "age".to_string(),
                typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                is_pk: false,
                is_autoincrement: false,
            },
        ],
    };

    let dst_age_col = PhysicalColumn {
        table_name: "ages".to_string(),
        column_name: "age".to_string(),
        typ: PhysicalColumnType::Int { bits: IntBits::_16 },
        is_pk: false,
        is_autoincrement: false,
    };

    let dst_table = PhysicalTable {
        name: "ages".to_string(),
        columns: vec![dst_age_col.clone()],
    };

    let src_name_phys_col = src_table.get_physical_column("name").unwrap();
    let src_age_phys_col = src_table.get_physical_column("age").unwrap();
    let src_age_col = src_table.get_column("age").unwrap();

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

    let lazy_col = Column::Lazy {
        row_index: 0,
        col_index: 0,
        step: &step_a,
    };
    let insertion_op = dst_table.insert(vec![&dst_age_col], vec![vec![&lazy_col]], vec![]);

    let step_b = TransactionStep {
        operation: SQLOperation::Insert(insertion_op),
        values: RefCell::new(vec![]),
    };

    let transaction_script = TransactionScript::Multi(vec![step_a.clone()], step_b);

    transaction_script
        .execute::<String>(&mut client, extractor)
        .unwrap();
}
