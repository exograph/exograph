use postgres::Row;

use super::SQLOperation;

pub enum TransactionScript<'a> {
    Single(SQLOperation<'a>),
    Multi(Vec<SQLOperation<'a>>),
}

pub enum TransactionScriptElement<'a> {
    Static(SQLOperation<'a>),
    Dynamic(Vec<SQLOperation<'a>>),
}

pub struct TransactionStep<'a, T> {
    pub operation: SQLOperation<'a>,
    pub extractor: fn(Vec<Row>) -> T,
}
