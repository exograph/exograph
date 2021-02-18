use postgres::types::ToSql;
use std::any::Any;

#[macro_use]
#[cfg(test)]
mod test_util;
mod database;
mod table;
mod column;
mod predicate;

pub trait SQLParam: ToSql + std::fmt::Display {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SQLParam) -> bool;
}

pub fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

impl<T: ToSql + Any + PartialEq + std::fmt::Display> SQLParam for T {
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
}

impl PartialEq for dyn SQLParam {
    fn eq(&self, other: &Self) -> bool {
        SQLParam::eq(self, other)
    }
}

pub struct ParameterBinding<'a> {
    stmt: String,
    params: Vec<&'a Box<dyn SQLParam>>
}

impl<'a> ParameterBinding<'a> {
    fn new(stmt: String, params: Vec<&'a Box<dyn SQLParam>>) -> Self {
        Self {
            stmt,
            params
        }
    }
}

trait Expression {
    fn binding(&self) -> ParameterBinding;
}

impl<T> Expression for Box<T> where T: Expression {
    fn binding(&self) -> ParameterBinding {
        self.as_ref().binding()
    }
}

