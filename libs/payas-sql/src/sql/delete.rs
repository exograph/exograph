use maybe_owned::MaybeOwned;

use super::{
    column::Column, physical_table::PhysicalTable, predicate::ConcretePredicate, Expression,
    SQLBuilder,
};

#[derive(Debug)]
pub struct Delete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> Expression for Delete<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str("DELETE FROM ");
        self.table.binding(builder);

        if self.predicate.as_ref() != &ConcretePredicate::True {
            builder.push_str(" WHERE ");
            self.predicate.binding(builder);
        }

        if !self.returning.is_empty() {
            builder.push_str(" RETURNING ");
            builder.push_iter(self.returning.iter(), ", ", |builder, elem| {
                elem.binding(builder)
            });
        }
    }
}

#[derive(Debug)]
pub struct TemplateDelete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: ConcretePredicate<'a>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

// TODO: Tie this properly to the prev_step
impl<'a> TemplateDelete<'a> {
    pub fn resolve(&'a self) -> Delete<'a> {
        let TemplateDelete {
            table,
            predicate,
            returning,
        } = self;

        Delete {
            table,
            predicate: predicate.into(),
            returning: returning
                .iter()
                .map(|c| MaybeOwned::Borrowed(c.as_ref()))
                .collect(),
        }
    }
}
