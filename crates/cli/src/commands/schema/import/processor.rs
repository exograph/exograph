use anyhow::Result;

use super::ImportContext;

pub(super) trait ModelProcessor {
    fn process(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()>;
}
