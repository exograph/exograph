use bytes::Bytes;
use maybe_owned::MaybeOwned;
use std::{
    any::Any,
    fmt::{Debug, Display},
    sync::Arc,
};
use tokio_postgres::types::{to_sql_checked, FromSql, ToSql, Type};

use crate::{database_error::DatabaseError, Ordering, PhysicalColumn, PhysicalTable};

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

#[derive(Debug)]
pub enum ParameterBinding<'a> {
    Literal(String),
    Table(&'a PhysicalTable),
    Column(&'a PhysicalColumn),
    Star(Option<&'a str>), // table name is optional
    PlainColumn(&'a PhysicalColumn),
    Function(String, Box<ParameterBinding<'a>>),
    Parameter(Arc<dyn SQLParam>),
    SubExpressions(Vec<ParameterBinding<'a>>),
    Static(&'static str),
    Cast(Box<ParameterBinding<'a>>, &'static str),
    JsonObject(Vec<(String, ParameterBinding<'a>)>),
    Parenthetical(Box<ParameterBinding<'a>>),
    Coalesce(Box<ParameterBinding<'a>>, &'static str),
    OrderByElement(&'a PhysicalColumn, Ordering),
    OrderBy(Vec<ParameterBinding<'a>>),
    Predicate(Box<ParameterBinding<'a>>),
    Boolean(bool),
    RelationalOperator(
        Box<ParameterBinding<'a>>,
        Box<ParameterBinding<'a>>,
        &'static str,
    ),
    LogicalOperator(
        Box<ParameterBinding<'a>>,
        Box<ParameterBinding<'a>>,
        &'static str,
    ),
    GroupBy(Vec<ParameterBinding<'a>>),
    LeftJoin(
        Box<ParameterBinding<'a>>,
        Box<ParameterBinding<'a>>,
        Box<ParameterBinding<'a>>,
    ),
    Select {
        columns: Vec<ParameterBinding<'a>>,
        from: Box<ParameterBinding<'a>>,
        predicate: Option<Box<ParameterBinding<'a>>>,
        group_by: Option<Box<ParameterBinding<'a>>>,
        order_by: Option<Box<ParameterBinding<'a>>>,
        limit: Option<Box<ParameterBinding<'a>>>,
        offset: Option<Box<ParameterBinding<'a>>>,
        alias: Option<String>,
        nested: bool,
    },
    Delete {
        table: Box<ParameterBinding<'a>>,
        predicate: Option<Box<ParameterBinding<'a>>>,
        returning: Vec<ParameterBinding<'a>>,
    },
    Insert {
        table: Box<ParameterBinding<'a>>,
        columns: Vec<ParameterBinding<'a>>,
        values: Vec<Vec<ParameterBinding<'a>>>,
        returning: Vec<ParameterBinding<'a>>,
    },
    Update {
        table: Box<ParameterBinding<'a>>,
        assignments: Vec<(ParameterBinding<'a>, ParameterBinding<'a>)>,
        predicate: Option<Box<ParameterBinding<'a>>>,
        returning: Vec<ParameterBinding<'a>>,
    },

    CteExpression {
        name: String,
        operation: Box<ParameterBinding<'a>>,
    },
    Cte {
        exprs: Vec<ParameterBinding<'a>>,
        select: Box<ParameterBinding<'a>>,
    },
}

impl<'a> ParameterBinding<'a> {
    pub fn string_expression(self) -> (String, Vec<Arc<dyn SQLParam>>) {
        let mut buffer = String::with_capacity(100);
        let mut params = Vec::new();
        self._string_expression(&mut buffer, &mut params);
        (buffer, params)
    }

    fn _string_expression(self, buffer: &mut String, params: &mut Vec<Arc<dyn SQLParam>>) {
        fn push_quoted(s: &str, buffer: &mut String) {
            buffer.push('\"');
            buffer.push_str(s);
            buffer.push('\"');
        }

        match self {
            ParameterBinding::Literal(l) => {
                buffer.push('\'');
                buffer.push_str(&l);
                buffer.push('\'');
            }
            ParameterBinding::Table(t) => {
                push_quoted(&t.name, buffer);
            }
            ParameterBinding::Column(c) => {
                push_quoted(&c.table_name, buffer);
                buffer.push('.');
                push_quoted(&c.column_name, buffer);
            }
            ParameterBinding::PlainColumn(c) => {
                push_quoted(&c.column_name, buffer);
            }
            ParameterBinding::Star(table_name) => {
                if let Some(table_name) = table_name {
                    push_quoted(table_name, buffer);
                    buffer.push('.');
                }
                buffer.push('*');
            }
            ParameterBinding::Function(function_name, function_param) => {
                buffer.push_str(&function_name);
                buffer.push('(');
                function_param._string_expression(buffer, params);
                buffer.push(')');
            }
            ParameterBinding::Parameter(param) => {
                params.push(param);
                buffer.push('$');
                buffer.push_str(&params.len().to_string());
            }
            ParameterBinding::Static(s) => buffer.push_str(s),
            ParameterBinding::Cast(elem, typ) => {
                elem._string_expression(buffer, params);
                buffer.push_str("::");
                buffer.push_str(typ);
            }
            ParameterBinding::JsonObject(elems) => {
                let elems_len = elems.len();

                buffer.push_str("json_build_object(");
                for (index, (key, value)) in elems.into_iter().enumerate() {
                    buffer.push('\'');
                    buffer.push_str(&key);
                    buffer.push('\'');
                    buffer.push_str(", ");
                    value._string_expression(buffer, params);
                    if index != elems_len - 1 {
                        buffer.push_str(", ");
                    }
                }
                buffer.push(')');
            }
            ParameterBinding::SubExpressions(elems) => {
                for elem in elems {
                    elem._string_expression(buffer, params);
                }
            }
            ParameterBinding::Parenthetical(elem) => {
                buffer.push('(');
                elem._string_expression(buffer, params);
                buffer.push(')');
            }
            ParameterBinding::Coalesce(elem, default) => {
                buffer.push_str("COALESCE(");
                elem._string_expression(buffer, params);
                buffer.push_str(", ");
                buffer.push_str(default);
                buffer.push(')');
            }

            ParameterBinding::OrderByElement(column, ordering) => {
                push_quoted(&column.table_name, buffer);
                buffer.push('.');
                push_quoted(&column.column_name, buffer);
                buffer.push(' ');
                if ordering == Ordering::Asc {
                    buffer.push_str("ASC");
                } else {
                    buffer.push_str("DESC");
                }
            }
            ParameterBinding::OrderBy(elems) => {
                let elems_len = elems.len();
                buffer.push_str("ORDER BY ");
                for (index, elem) in elems.into_iter().enumerate() {
                    elem._string_expression(buffer, params);
                    if index != elems_len - 1 {
                        buffer.push_str(", ");
                    }
                }
            }
            ParameterBinding::Predicate(elem) => {
                buffer.push_str("WHERE ");
                elem._string_expression(buffer, params);
            }
            ParameterBinding::Boolean(b) => {
                if b {
                    buffer.push_str("TRUE");
                } else {
                    buffer.push_str("FALSE");
                }
            }
            ParameterBinding::RelationalOperator(left, right, op) => {
                left._string_expression(buffer, params);
                buffer.push(' ');
                buffer.push_str(op);
                buffer.push(' ');
                right._string_expression(buffer, params);
            }
            ParameterBinding::LogicalOperator(left, right, op) => {
                buffer.push('(');
                left._string_expression(buffer, params);
                buffer.push(' ');
                buffer.push_str(op);
                buffer.push(' ');
                right._string_expression(buffer, params);
                buffer.push(')');
            }
            ParameterBinding::GroupBy(elems) => {
                let elems_len = elems.len();
                buffer.push_str("GROUP BY ");
                for (index, elem) in elems.into_iter().enumerate() {
                    elem._string_expression(buffer, params);
                    if index != elems_len - 1 {
                        buffer.push_str(", ");
                    }
                }
            }
            ParameterBinding::LeftJoin(left, right, on) => {
                left._string_expression(buffer, params);
                buffer.push_str(" LEFT JOIN ");
                right._string_expression(buffer, params);
                buffer.push_str(" ON ");
                on._string_expression(buffer, params);
            }
            ParameterBinding::Select {
                columns,
                from,
                predicate: filter,
                group_by,
                order_by,
                limit,
                offset,
                alias,
                nested,
            } => {
                if nested {
                    buffer.push('(');
                }
                buffer.push_str("SELECT ");
                let columns_len = columns.len();
                for (index, column) in columns.into_iter().enumerate() {
                    column._string_expression(buffer, params);
                    if index != columns_len - 1 {
                        buffer.push_str(", ");
                    }
                }
                buffer.push_str(" FROM ");
                from._string_expression(buffer, params);
                if let Some(filter) = filter {
                    buffer.push(' ');
                    filter._string_expression(buffer, params);
                }
                if let Some(group_by) = group_by {
                    buffer.push(' ');
                    group_by._string_expression(buffer, params);
                }
                if let Some(order_by) = order_by {
                    buffer.push(' ');
                    order_by._string_expression(buffer, params);
                }
                if let Some(limit) = limit {
                    buffer.push_str(" LIMIT ");
                    limit._string_expression(buffer, params);
                }
                if let Some(offset) = offset {
                    buffer.push_str(" OFFSET ");
                    offset._string_expression(buffer, params);
                }
                if let Some(alias) = alias {
                    buffer.push_str(" AS ");
                    push_quoted(&alias, buffer);
                }
                if nested {
                    buffer.push(')');
                }
            }
            ParameterBinding::Delete {
                table,
                predicate,
                returning,
            } => {
                buffer.push_str("DELETE FROM ");
                table._string_expression(buffer, params);

                if let Some(predicate) = predicate {
                    buffer.push_str(" WHERE ");
                    predicate._string_expression(buffer, params);
                }

                if !returning.is_empty() {
                    buffer.push_str(" RETURNING ");
                    let returning_len = returning.len();
                    for (index, elem) in returning.into_iter().enumerate() {
                        elem._string_expression(buffer, params);
                        if index != returning_len - 1 {
                            buffer.push_str(", ");
                        }
                    }
                }
            }
            ParameterBinding::Insert {
                table,
                columns,
                values,
                returning,
            } => {
                buffer.push_str("INSERT INTO ");
                table._string_expression(buffer, params);
                buffer.push_str(" (");
                let columns_len = columns.len();
                for (index, column) in columns.into_iter().enumerate() {
                    column._string_expression(buffer, params);
                    if index != columns_len - 1 {
                        buffer.push_str(", ");
                    }
                }
                buffer.push_str(") VALUES (");
                let valuess_len = values.len();
                for (index, values) in values.into_iter().enumerate() {
                    let values_len = values.len();
                    for (index, value) in values.into_iter().enumerate() {
                        value._string_expression(buffer, params);
                        if index != values_len - 1 {
                            buffer.push_str(", ");
                        }
                    }
                    if index != valuess_len - 1 {
                        buffer.push_str("), (");
                    }
                }
                buffer.push(')');

                if !returning.is_empty() {
                    buffer.push_str(" RETURNING ");
                    let returning_len = returning.len();
                    for (index, elem) in returning.into_iter().enumerate() {
                        elem._string_expression(buffer, params);
                        if index != returning_len - 1 {
                            buffer.push_str(", ");
                        }
                    }
                }
            }
            ParameterBinding::Update {
                table,
                assignments,
                predicate,
                returning,
            } => {
                buffer.push_str("UPDATE ");
                table._string_expression(buffer, params);
                buffer.push_str(" SET ");
                let assignments_len = assignments.len();
                for (index, (assignment_col, assignment_value)) in
                    assignments.into_iter().enumerate()
                {
                    assignment_col._string_expression(buffer, params);
                    buffer.push_str(" = ");
                    assignment_value._string_expression(buffer, params);

                    if index != assignments_len - 1 {
                        buffer.push_str(", ");
                    }
                }
                if let Some(predicate) = predicate {
                    buffer.push_str(" WHERE ");
                    predicate._string_expression(buffer, params);
                }

                if !returning.is_empty() {
                    buffer.push_str(" RETURNING ");
                    let returning_len = returning.len();
                    for (index, elem) in returning.into_iter().enumerate() {
                        elem._string_expression(buffer, params);
                        if index != returning_len - 1 {
                            buffer.push_str(", ");
                        }
                    }
                }
            }
            ParameterBinding::CteExpression { name, operation } => {
                push_quoted(&name, buffer);
                buffer.push_str(" AS (");
                operation._string_expression(buffer, params);
                buffer.push(')');
            }
            ParameterBinding::Cte { exprs, select } => {
                buffer.push_str("WITH ");
                let exprs_len = exprs.len();
                exprs.into_iter().enumerate().for_each(|(index, expr)| {
                    expr._string_expression(buffer, params);
                    if index != exprs_len - 1 {
                        buffer.push_str(", ");
                    }
                });
                buffer.push(' ');
                select._string_expression(buffer, params);
            }
        }
    }
}

// #[derive(Debug, Clone)]
// pub struct ParameterBinding<'a> {
//     elems: Vec<ExpressionElement<'a>>,
//     // pub stmt: String,
//     // pub params: Vec<&'a (dyn SQLParam + 'static)>,
// }

// impl<'a> ParameterBinding<'a> {
//     fn new(stmt: String, params: Vec<&'a (dyn SQLParam + 'static)>) -> Self {
//         Self { stmt, params }
//     }

//     fn tupled(self) -> (String, Vec<&'a (dyn SQLParam + 'static)>) {
//         (self.stmt, self.params)
//     }
// }

pub trait OperationExpression {
    fn binding(&self) -> ParameterBinding;
}

pub trait Expression {
    fn binding(&self) -> ParameterBinding;
}

impl<T> Expression for Box<T>
where
    T: Expression,
{
    fn binding(&self) -> ParameterBinding {
        self.as_ref().binding()
    }
}

impl<'a, T> Expression for MaybeOwned<'a, T>
where
    T: Expression,
{
    fn binding(&self) -> ParameterBinding {
        self.as_ref().binding()
    }
}
