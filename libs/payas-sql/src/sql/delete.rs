use maybe_owned::MaybeOwned;

use super::{
    column::Column, physical_table::PhysicalTable, predicate::ConcretePredicate, Expression,
    ParameterBinding,
};

#[derive(Debug)]
pub struct Delete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> Expression for Delete<'a> {
    fn binding(&self) -> ParameterBinding {
        let table_binding = ParameterBinding::Table(self.table);

        let predicate_binding = if self.predicate.as_ref() != &ConcretePredicate::True {
            Some(Box::new(self.predicate.binding()))
        } else {
            None
        };

        ParameterBinding::Delete {
            table: Box::new(table_binding),
            predicate: predicate_binding,
            returning: self.returning.iter().map(|ret| ret.binding()).collect(),
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
