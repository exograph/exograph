use maybe_owned::MaybeOwned;

#[cfg(test)]
use crate::SQLParam;
#[cfg(test)]
use std::sync::Arc;

use super::SQLBuilder;

/// A trait for types that can build themselves into an SQL expression.
///
/// Each constituent of an SQL expression (column, table, function, select, etc.) should implement
/// this trait, which can then be used to hierarchically build an SQL string and the list of
/// parameters to be supplied to it.
pub trait ExpressionBuilder {
    /// Build the SQL expression into the given SQL builder
    fn build(&self, builder: &mut SQLBuilder);

    /// Build the SQL expression into a string and return it This is useful for testing, where we
    /// want to assert on the generated SQL without going through the whole process of creating an
    /// SQLBuilder, then building the SQL expression into it, and finally extracting the SQL string
    /// and params.
    #[cfg(test)]
    fn into_sql(self) -> (String, Vec<Arc<dyn SQLParam>>)
    where
        Self: Sized,
    {
        let mut builder = SQLBuilder::new();
        self.build(&mut builder);
        builder.into_sql()
    }
}

impl<T> ExpressionBuilder for Box<T>
where
    T: ExpressionBuilder,
{
    fn build(&self, builder: &mut SQLBuilder) {
        self.as_ref().build(builder)
    }
}

impl<'a, T> ExpressionBuilder for MaybeOwned<'a, T>
where
    T: ExpressionBuilder,
{
    fn build(&self, builder: &mut SQLBuilder) {
        self.as_ref().build(builder)
    }
}

impl<T> ExpressionBuilder for &T
where
    T: ExpressionBuilder,
{
    fn build(&self, builder: &mut SQLBuilder) {
        (**self).build(builder)
    }
}
