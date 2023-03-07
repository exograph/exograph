use bytes::Bytes;
use maybe_owned::MaybeOwned;
use std::{
    any::Any,
    fmt::{Debug, Display},
    sync::Arc,
};
use tokio_postgres::types::{to_sql_checked, FromSql, ToSql, Type};

use crate::database_error::DatabaseError;

#[macro_use]
#[cfg(test)]
mod test_util;

pub mod column;
pub(crate) mod cte;
pub mod database;
pub(crate) mod delete;
pub(crate) mod insert;
pub(crate) mod physical_table;
pub(crate) mod select;
pub(crate) mod sql_operation;

pub mod array_util;
pub(crate) mod group_by;
mod join;
pub(crate) mod limit;
pub(crate) mod offset;
pub mod order;
pub mod predicate;
pub(crate) mod table;
pub(crate) mod transaction;
pub(crate) mod update;

pub trait SQLParam: ToSql + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SQLParam) -> bool;

    fn as_pg(&self) -> &(dyn ToSql + Sync);
}

impl<T: ToSql + Send + Sync + Any + PartialEq> SQLParam for T {
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

/// An SQL value to transfer result of a step to another
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SQLValue {
    value: Vec<u8>,
    type_: Type,
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
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        if *ty == self.type_ {
            out.extend(self.value.as_slice());
            Ok(tokio_postgres::types::IsNull::No)
        } else {
            Err(DatabaseError::Validation("Type mismatch".into()).into())
        }
    }

    fn accepts(_ty: &Type) -> bool
    where
        Self: Sized,
    {
        true
    }

    to_sql_checked!();
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>>
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

    to_sql_checked!();
}

/// A wrapper type for SQL parameters that can be used in a prepared statement.
/// We would have been fine with just using `Arc<dyn SQLParam>` but we need to
/// implement `ToSql` for it and since `Arc` (unlike `Box`) is not a `#[fundamental]`
/// type, so we have to wrap it in a newtype.
#[derive(Clone)]
pub struct SQLParamContainer(Arc<dyn SQLParam>);

impl SQLParamContainer {
    pub fn param(&self) -> Arc<dyn SQLParam> {
        self.0.clone()
    }
}

impl ToSql for SQLParamContainer {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.0.as_ref().to_sql_checked(ty, out)
    }

    fn accepts(_ty: &Type) -> bool {
        true // TODO: Can we check this?
    }

    to_sql_checked!();
}

impl SQLParamContainer {
    pub fn new<T: SQLParam + 'static>(param: T) -> Self {
        Self(Arc::new(param))
    }
}

impl PartialEq for SQLParamContainer {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl AsRef<dyn SQLParam> for SQLParamContainer {
    fn as_ref(&self) -> &(dyn SQLParam + 'static) {
        self.0.as_ref()
    }
}

impl Debug for SQLParamContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Display for SQLParamContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Display for SQLBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct SQLBuilder {
    sql: String,
    params: Vec<Arc<dyn SQLParam>>,
    plain: bool, // Indicates if column name should be rendered without the table name i.e. "col" instead of "table"."col" (needed for INSERT statements)
}

impl SQLBuilder {
    pub fn new() -> Self {
        Self {
            sql: String::new(),
            params: Vec::new(),
            plain: false,
        }
    }

    pub fn push_str<T: AsRef<str>>(&mut self, s: T) {
        self.sql.push_str(s.as_ref());
    }

    pub fn push(&mut self, c: char) {
        self.sql.push(c);
    }

    pub fn push_quoted<T: AsRef<str>>(&mut self, s: T) {
        self.sql.push('"');
        self.sql.push_str(s.as_ref());
        self.sql.push('"');
    }

    pub fn push_param(&mut self, param: Arc<dyn SQLParam>) {
        self.params.push(param);
        self.push('$');
        self.push_str(&self.params.len().to_string());
    }

    pub fn push_iter<T>(
        &mut self,
        iter: impl ExactSizeIterator<Item = T>,
        sep: &str,
        mapping: impl Fn(&mut Self, T),
    ) {
        let len = iter.len();
        for (i, item) in iter.enumerate() {
            mapping(self, item);
            if i < len - 1 {
                self.sql.push_str(sep);
            }
        }
    }

    pub fn push_elems<T: Expression>(&mut self, elems: &[T], sep: &str) {
        self.push_iter(elems.iter(), sep, |builder, elem| {
            elem.binding(builder);
        });
    }

    pub fn into_sql(self) -> (String, Vec<Arc<dyn SQLParam>>) {
        (self.sql, self.params)
    }

    fn with_plain<F, R>(&mut self, func: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let cur_plain = self.plain;
        self.plain = true;
        let ret = func(self);
        self.plain = cur_plain;
        ret
    }
}

pub trait Expression {
    fn binding(&self, builder: &mut SQLBuilder);

    #[cfg(test)]
    fn into_sql(self) -> (String, Vec<Arc<dyn SQLParam>>)
    where
        Self: Sized,
    {
        let mut builder = SQLBuilder::new();
        self.binding(&mut builder);
        builder.into_sql()
    }
}

impl<T> Expression for Box<T>
where
    T: Expression,
{
    fn binding(&self, builder: &mut SQLBuilder) {
        self.as_ref().binding(builder)
    }
}

impl<'a, T> Expression for MaybeOwned<'a, T>
where
    T: Expression,
{
    fn binding(&self, builder: &mut SQLBuilder) {
        self.as_ref().binding(builder)
    }
}

impl<T> Expression for &T
where
    T: Expression,
{
    fn binding(&self, builder: &mut SQLBuilder) {
        (**self).binding(builder)
    }
}
