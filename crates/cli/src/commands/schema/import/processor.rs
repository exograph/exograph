use anyhow::Result;

use super::ImportContext;

pub(super) const INDENT: &str = "  ";

/// A trait for processing a model.
///
/// `P` is the parent type.
/// `PC` is the parent context type.
///
/// The parent context is used to pass information from the parent to the child.
/// For example, if the parent is a table, the parent context could be a set of
/// fields that have already been added to the model.
pub(super) trait ModelProcessor<P, PC> {
    fn process(
        &self,
        parent: &P,
        context: &ImportContext,
        parent_context: &mut PC,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()>;
}
