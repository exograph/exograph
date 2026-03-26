// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;
use exo_sql_core::operation::{ColumnPredicate, DatabaseExtension, OrderBy, Select};

use crate::{
    order_by::AbstractOrderBy, predicate::AbstractPredicate, select::AbstractSelect,
    selection_level::SelectionLevel,
};

pub trait SelectTransformer<Ext: DatabaseExtension> {
    fn to_select(&self, abstract_select: AbstractSelect<Ext>, database: &Database) -> Select<Ext>;
}

pub trait PredicateTransformer<Ext: DatabaseExtension> {
    /// Transform an abstract predicate into a concrete predicate
    ///
    /// # Arguments
    /// * `predicate` - The predicate to transform
    /// * `selection_level` - The selection level of that led to this predicate (through subselects)
    /// * `assume_tables_in_context` - Whether the tables are already in context. If they are, the predicate can simply use the table.column syntax.
    ///   If they are not, the predicate will need to bring in the tables being referred to.
    /// * `database` - The database
    fn to_predicate(
        &self,
        predicate: &AbstractPredicate<Ext>,
        selection_level: &SelectionLevel,
        assume_tables_in_context: bool,
        database: &Database,
    ) -> ColumnPredicate<Ext>;
}

pub trait OrderByTransformer<Ext: DatabaseExtension> {
    fn to_order_by(
        &self,
        order_by: &AbstractOrderBy<Ext>,
        selection_level: &SelectionLevel,
        database: &Database,
    ) -> OrderBy<Ext>;
}
