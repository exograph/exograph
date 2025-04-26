use anyhow::Result;

use super::ImportContext;

pub(super) const INDENT: &str = "  ";

pub(super) trait ModelProcessor<P> {
    fn process(
        &self,
        parent: &P,
        context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()>;
}
