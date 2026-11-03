use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that purges expired rip history records older than the configured threshold.
#[derive(Debug, Default, Clone, Copy)]
pub struct HistoryPurgePostProcessor;

impl PostProcessor for HistoryPurgePostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if let Some(days) = ctx.auto_cleanup_days {
            let _ = crate::history::purge_expired_rip_records(days, None);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_history_purge_post_processor_none() {
        let processor = HistoryPurgePostProcessor;
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        assert!(processor.process(&ctx).is_ok());
    }
}
