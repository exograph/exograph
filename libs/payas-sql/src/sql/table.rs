use crate::PhysicalTable;

use super::{
    join::LeftJoin, predicate::ConcretePredicate, select::Select, ExpressionBuilder, SQLBuilder,
};
use maybe_owned::MaybeOwned;

/// A table-like concept that can be used in in place of `SELECT FROM <table-query> ...`.
#[derive(Debug, PartialEq)]
pub enum Table<'a> {
    /// A physical table such as `concerts`.
    Physical(&'a PhysicalTable),
    /// A join between two tables such as `concerts LEFT JOIN venues ON concerts.venue_id = venues.id`.
    Join(LeftJoin<'a>),
    /// A sub-select such as `(SELECT * FROM concerts) AS concerts`.
    SubSelect {
        select: Box<Select<'a>>,
        /// The alias of the sub-select (optional, since we need to alias the sub-select when used in a FROM clause)
        alias: Option<String>,
    },
}

impl<'a> Table<'a> {
    pub fn join(
        self,
        other_table: Table<'a>,
        predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    ) -> Table<'a> {
        Table::Join(LeftJoin::new(self, other_table, predicate))
    }
}

impl<'a> ExpressionBuilder for Table<'a> {
    /// Build the table into a SQL string.
    fn build(&self, builder: &mut SQLBuilder) {
        match self {
            Table::Physical(physical_table) => builder.push_identifier(&physical_table.name),
            Table::Join(join) => join.build(builder),
            Table::SubSelect { select, alias } => {
                builder.push('(');
                select.build(builder);
                builder.push(')');
                if let Some(alias) = alias {
                    builder.push_str(" AS ");
                    builder.push_identifier(alias);
                }
            }
        }
    }
}
