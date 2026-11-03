use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that executes a user-specified post-processing shell script.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScriptHookPostProcessor;

impl PostProcessor for ScriptHookPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if let (Some(script), Some(out_file)) =
            (ctx.post_script.as_deref(), ctx.output_file.as_ref())
        {
            let _ = crate::utils::run_post_processing_hook(
                script,
                out_file,
                &ctx.title,
                &ctx.media_type,
                ctx.year,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_script_hook_post_processor_none() {
        let processor = ScriptHookPostProcessor;
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        assert!(processor.process(&ctx).is_ok());
    }
}
