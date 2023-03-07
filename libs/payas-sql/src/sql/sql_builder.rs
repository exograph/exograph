use std::sync::Arc;

use crate::SQLParam;

use super::ExpressionBuilder;

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

    pub fn in_plain_mode(&self) -> bool {
        self.plain
    }

    /// Push a string
    pub fn push_str<T: AsRef<str>>(&mut self, s: T) {
        self.sql.push_str(s.as_ref());
    }

    /// Push a character
    pub fn push(&mut self, c: char) {
        self.sql.push(c);
    }

    /// Push a string surrounded by double quotes. Useful for identifier.
    pub fn push_identifier<T: AsRef<str>>(&mut self, s: T) {
        self.sql.push('"');
        self.sql.push_str(s.as_ref());
        self.sql.push('"');
    }

    /// Push a parameter, which will be replaced with a placeholder in the SQL string
    /// and the parameter will be added to the list of parameters.
    pub fn push_param(&mut self, param: Arc<dyn SQLParam>) {
        self.params.push(param);
        self.push('$');
        self.push_str(&self.params.len().to_string());
    }

    /// Push elements of an iterator, separated by `sep`. The `mapping` function provides
    /// the flexibility to map the elements (compared to [`SQLBuilder::push_elems`], which assumes that
    /// the elements implement [`ExpressionBuilder`] and [`build`](ExpressionBuilder::build) is all you need to call).
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

    /// Push elements of a slice, separated by `sep`. The elements must themselves implement
    /// `ExpressionBuilder`. This is a convenience method that encodes the common pattern of
    /// building a list of expressions and separating them by a separator.
    pub fn push_elems<T: ExpressionBuilder>(&mut self, elems: &[T], sep: &str) {
        self.push_iter(elems.iter(), sep, |builder, elem| {
            elem.build(builder);
        });
    }

    /// Get the SQL string and the list of parameters. Calling this method should be the final step
    /// in building an SQL expression, and thus this builder consumes the `self`.
    pub fn into_sql(self) -> (String, Vec<Arc<dyn SQLParam>>) {
        (self.sql, self.params)
    }

    /// Execute the given function with the `plain` flag set to true. This is useful for building
    /// SQL expressions that need to be rendered without the table name, e.g. for INSERT and UPDATE
    /// statements. This takes a closure, so that we can restore the original value of the `plain`
    /// flag after the function has been executed.
    pub fn with_plain<F, R>(&mut self, func: F) -> R
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
