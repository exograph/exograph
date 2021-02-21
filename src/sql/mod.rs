use postgres::types::ToSql;
use std::any::Any;

#[macro_use]
#[cfg(test)]
mod test_util;
pub mod column;
pub mod database;
pub mod table;

pub mod order;
pub mod predicate;

pub trait SQLParam: ToSql + Sync + std::fmt::Display {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SQLParam) -> bool;

    fn as_pg(&self) -> &(dyn ToSql + Sync);
}

pub fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

impl<T: ToSql + Sync + Any + PartialEq + std::fmt::Display> SQLParam for T {
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

#[derive(Debug, Clone)]
pub struct ParameterBinding<'a> {
    pub stmt: String,
    pub params: Vec<&'a Box<dyn SQLParam>>,
}

impl<'a> ParameterBinding<'a> {
    fn new(stmt: String, params: Vec<&'a Box<dyn SQLParam>>) -> Self {
        Self { stmt, params }
    }
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

pub struct ExpressionContext {
    param_count: u16,
}

impl ExpressionContext {
    pub fn new() -> Self {
        Self { param_count: 0 }
    }

    pub fn next_param(&mut self) -> u16 {
        self.param_count += 1;

        self.param_count
    }
}
