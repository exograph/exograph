use postgres::{types::Type, Row};

use super::{SQLDynamicOperation, SQLOperation, SQLValue};

pub enum TransactionScript<'a> {
    Single(SQLOperation<'a>),
    Multi(Vec<SQLOperation<'a>>),
}

#[derive(Debug)]
pub enum TransactionScriptX<'a> {
    Single(TransactionStep<'a>),
    Multi(Vec<TransactionStep<'a>>),
}

#[derive(Debug)]
pub enum TransactionScriptElement<'a> {
    Static(SQLOperation<'a>),
    Dynamic(SQLDynamicOperation<'a>),
}

#[derive(Debug)]
pub struct TransactionStep<'a> {
    pub operation: TransactionScriptElement<'a>,
    pub pg_result_types: Vec<Type>,
    pub extractor: fn(Row) -> RowResult<'a>,
    pub result: fn() -> TransactionStepResult<'a>,
}

type RowResult<'a> = &'a [&'a SQLValue<'a>];
type TransactionStepResult<'a> = &'a [RowResult<'a>];
