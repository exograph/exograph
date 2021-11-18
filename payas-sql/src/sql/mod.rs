use anyhow::anyhow;
use bytes::Bytes;
use maybe_owned::MaybeOwned;
use postgres::types::{FromSql, ToSql, Type};
use std::{
    any::Any,
    fmt::{Debug, Display},
};

#[macro_use]
#[cfg(test)]
mod test_util;
pub mod column;
mod cte;
pub mod database;
mod delete;
mod insert;
mod physical_table;
mod select;
mod sql_operation;

mod join;
mod limit;
mod offset;
pub mod order;
pub mod predicate;
mod table;
pub mod transaction;
mod update;

pub use cte::Cte;
pub use delete::{Delete, TemplateDelete};
pub use insert::{Insert, TemplateInsert};
pub use limit::Limit;
pub use offset::Offset;
pub use physical_table::PhysicalTable;
pub use select::Select;
pub use sql_operation::{SQLOperation, TemplateSQLOperation};
pub use table::TableQuery;
pub use update::{TemplateUpdate, Update};

pub trait SQLParam: ToSql + Sync + Display {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SQLParam) -> bool;

    fn as_pg(&self) -> &(dyn ToSql + Sync);
}

impl<T: ToSql + Sync + Any + PartialEq + Display> SQLParam for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn SQLParam) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<T>() {
            self == other
        } else {
            false
        }
    }

    fn as_pg(&self) -> &(dyn ToSql + Sync) {
        self
    }
}

impl PartialEq for dyn SQLParam {
    fn eq(&self, other: &Self) -> bool {
        SQLParam::eq(self, other)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SQLValue {
    value: Vec<u8>,
    type_: Type,
}

impl SQLValue {
    fn as_sql_param(&self) -> &dyn SQLParam {
        self
    }
}

impl Display for SQLValue {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "<SQLValue containing {}>", self.type_)
    }
}

impl ToSql for SQLValue {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut postgres::types::private::BytesMut,
    ) -> Result<postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        if *ty == self.type_ {
            out.extend(self.value.as_slice());
            Ok(postgres::types::IsNull::No)
        } else {
            Err(anyhow!("Type mismatch").into())
        }
    }

    fn accepts(_ty: &Type) -> bool
    where
        Self: Sized,
    {
        true
    }

    postgres::types::to_sql_checked!();
}

impl<'a> FromSql<'a> for SQLValue {
    fn from_sql(ty: &Type, raw: &[u8]) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(SQLValue {
            value: raw.to_owned(), // TODO: do we need to do this?
            type_: ty.clone(),
        })
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

// Wrapper type for bytes::Bytes for use with BYTEA
// Bytes does not implement ToSql.
#[derive(Debug, Clone, PartialEq)]
pub struct SQLBytes(pub Bytes);

impl SQLBytes {
    pub fn new(vec: Vec<u8>) -> Self {
        Self(Bytes::from(vec))
    }
}

impl ToSql for SQLBytes {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        (&self.0[..]).to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool
    where
        Self: Sized,
    {
        matches!(*ty, Type::BYTEA)
    }

    postgres::types::to_sql_checked!();
}

impl Display for SQLBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct ParameterBinding<'a> {
    pub stmt: String,
    pub params: Vec<&'a (dyn SQLParam + 'static)>,
}

impl<'a> ParameterBinding<'a> {
    fn new(stmt: String, params: Vec<&'a (dyn SQLParam + 'static)>) -> Self {
        Self { stmt, params }
    }

    fn tupled(self) -> (String, Vec<&'a (dyn SQLParam + 'static)>) {
        (self.stmt, self.params)
    }
}

pub trait OperationExpression {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding;
}

pub trait Expression {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding;
}

impl<T> Expression for Box<T>
where
    T: Expression,
{
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        self.as_ref().binding(expression_context)
    }
}

impl<'a, T> Expression for MaybeOwned<'a, T>
where
    T: Expression,
{
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        self.as_ref().binding(expression_context)
    }
}

#[derive(Default)]
pub struct ExpressionContext {
    param_count: u16,
    plain: bool, // Indicates if column name should be rendered without the table name i.e. "col" instead of "table"."col"
}

impl ExpressionContext {
    pub fn next_param(&mut self) -> u16 {
        self.param_count += 1;

        self.param_count
    }

    fn with_plain<F, R>(&mut self, func: F) -> R
    where
        F: FnOnce(&mut ExpressionContext) -> R,
    {
        let cur_plain = self.plain;
        self.plain = true;
        let ret = func(self);
        self.plain = cur_plain;
        ret
    }
}

pub fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}
