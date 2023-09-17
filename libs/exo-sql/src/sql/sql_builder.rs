// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, sync::Arc};

use crate::{Database, SQLParam};

use super::ExpressionBuilder;

pub struct SQLBuilder {
    /// The SQL being built with placeholders for each parameter
    sql: String,
    /// The list of parameters
    params: Vec<Arc<dyn SQLParam>>,
    /// Indicates if column name should be rendered with the table name i.e. "table"."col"  instead
    /// of "col" (needed for INSERT/UPDATE statements)
    fully_qualify_column_names: bool,
    // Map from CTE name to table name. This is used to render CTE names in lieu of table names for the `select` in a CTE.
    cte_table_map: HashMap<String, String>,
}

impl SQLBuilder {
    pub fn new() -> Self {
        Self {
            sql: String::new(),
            params: Vec::new(),
            fully_qualify_column_names: true,
            cte_table_map: HashMap::new(),
        }
    }

    /// Push a string
    pub fn push_str<T: AsRef<str>>(&mut self, s: T) {
        self.sql.push_str(s.as_ref());
    }

    /// Push a character
    pub fn push(&mut self, c: char) {
        self.sql.push(c);
    }

    /// Push a string surrounded by double quotes. Useful for identifier such as table names, column
    /// names, etc. Without the quotes, the identifier with uppercase letters will be interpreted
    /// the same as the identifier with lowercase letters.
    pub fn push_identifier<T: AsRef<str>>(&mut self, s: T) {
        self.sql.push('"');
        self.sql.push_str(s.as_ref());
        self.sql.push('"');
    }

    pub fn push_column_with_table_alias<T1: AsRef<str>, T2: AsRef<str>>(
        &mut self,
        column_name: T1,
        table_alias: T2,
    ) {
        self.push_identifier(table_alias);
        self.push('.');
        self.push_identifier(column_name);
    }

    /// Push a table name. If the table name is a CTE name, push the CTE name instead.
    pub fn push_table<T: AsRef<str>>(&mut self, table_name: T) {
        let table_name = table_name.as_ref();
        let table_name = match self.cte_table_map.get(table_name) {
            Some(cte_name) => cte_name,
            None => table_name,
        }
        .to_owned();
        self.push_identifier(table_name);
    }

    /// Push a table prefix (for a column). Push `<table_name>.` if in fully_qualify_column_names
    /// mode, otherwise an empty string.
    pub fn push_table_prefix<T: AsRef<str>>(&mut self, table_name: T) {
        if self.fully_qualify_column_names {
            self.push_table(table_name);
            self.push('.');
        }
    }

    /// Push a space. This is a common operation, so it is provided as a separate method.
    pub fn push_space(&mut self) {
        self.sql.push(' ');
    }

    /// Push a parameter, which will be replaced with a placeholder in the SQL string
    /// and the parameter will be added to the list of parameters.
    pub fn push_param(&mut self, param: Arc<dyn SQLParam>) {
        self.params.push(param);
        self.push('$');
        self.push_str(&self.params.len().to_string());
    }

    /// Push elements of an iterator, separated by `sep`. The `push_elem` function provides
    /// the flexibility to map the elements (compared to [`SQLBuilder::push_elems`], which assumes that
    /// the elements implement [`ExpressionBuilder`] and [`build`](ExpressionBuilder::build) is all you need to call).
    pub fn push_iter<T>(
        &mut self,
        iter: impl ExactSizeIterator<Item = T>,
        sep: &str,
        push_elem: impl Fn(&mut Self, T),
    ) {
        let len = iter.len();
        for (i, item) in iter.enumerate() {
            push_elem(self, item);

            if i < len - 1 {
                self.sql.push_str(sep);
            }
        }
    }

    /// Push elements of a slice, separated by `sep`. The elements must themselves implement
    /// `ExpressionBuilder`. This is a convenience method that encodes the common pattern of
    /// building a list of expressions and separating them by a separator.
    pub fn push_elems<T: ExpressionBuilder>(
        &mut self,
        database: &Database,
        elems: &[T],
        sep: &str,
    ) {
        self.push_iter(elems.iter(), sep, |builder, elem| {
            elem.build(database, builder);
        });
    }

    /// Get the SQL string and the list of parameters. Calling this method should be the final step
    /// in building an SQL expression, and thus this builder consumes the `self`.
    pub fn into_sql(self) -> (String, Vec<Arc<dyn SQLParam>>) {
        (self.sql, self.params)
    }

    /// Execute the given function with the [`Self::fully_qualify_column_names`] flag set to false.
    /// This is useful for building SQL expressions that need to be rendered without the table name,
    /// e.g. for INSERT and UPDATE statements. This takes a closure, so that we can restore the
    /// original value of the flag after executing the function.
    pub fn without_fully_qualified_column_names<F, R>(&mut self, func: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let cur_fully_qualify_column_names = self.fully_qualify_column_names;
        self.fully_qualify_column_names = false;
        let ret = func(self);
        self.fully_qualify_column_names = cur_fully_qualify_column_names;
        ret
    }

    pub fn with_cte_table_map<F, R>(&mut self, cte_table_map: HashMap<String, String>, func: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let cur_cte_table_map = self.cte_table_map.clone();
        self.cte_table_map.extend(cte_table_map);
        let ret = func(self);
        self.cte_table_map = cur_cte_table_map;
        ret
    }
}
