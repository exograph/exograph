use anyhow::Result;

use super::ImportContext;

pub(super) const INDENT: &str = "  ";

/// A trait for processing a model.
///
/// `P` is the parent type.
pub(super) trait ModelProcessor<P> {
    fn process(
        &self,
        parent: &P,
        context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()>;
}
