pub mod delete_transformer;
mod insert_transformer;
mod order_by_transformer;
mod predicate_transformer;
mod select;
mod update_transformer;

pub struct Postgres {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionLevel {
    TopLevel,
    Nested,
}
