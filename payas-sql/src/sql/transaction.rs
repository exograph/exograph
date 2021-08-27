use postgres::{types::Type, Row};

use super::{SQLOperation, SQLParam, SQLValue};

pub enum TransactionScript<'a> {
    Single(SQLOperation<'a>),
    Multi(Vec<SQLOperation<'a>>),
}

pub enum TransactionScriptX<'a> {
    Single(TransactionStep<'a>),
    Multi(Vec<TransactionStep<'a>>),
}

pub enum TransactionScriptElement<'a> {
    Static(SQLOperation<'a>),
    Dynamic(Vec<SQLOperation<'a>>),
}

pub struct TransactionStep<'a> {
    pub operation: TransactionScriptElement<'a>,
    pub pg_result_types: Vec<Type>,
    pub extractor: fn(Vec<Row>) -> Vec<Vec<&'a SQLValue<'a>>>, // FromSql + ToSql
}
