use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that calculates SHA-256 digests and saves a .sha256 sidecar file.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChecksumPostProcessor;

impl PostProcessor for ChecksumPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if ctx.checksum_enabled
            && let Some(ref out_file) = ctx.output_file
        {
            let _ = crate::utils::generate_checksum_file(out_file);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_checksum_post_processor_disabled() {
        let processor = ChecksumPostProcessor;
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        ctx.checksum_enabled = false;
        assert!(processor.process(&ctx).is_ok());
    }
}
