use crate::ast::ast_types::FieldSelection;

use super::{Type, Typed};

impl FieldSelection<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            FieldSelection::Single(_, typ) => typ,
            FieldSelection::Select(_, _, _, typ) => typ,
        }
    }
}
