use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that appends an audit log entry in JSON format.
#[derive(Debug, Default, Clone, Copy)]
pub struct AuditLogPostProcessor;

impl PostProcessor for AuditLogPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if let Some(ref audit_path) = ctx.audit_log_path {
            let details = serde_json::json!({
                "title": ctx.title,
                "year": ctx.year,
                "dvd_path": ctx.dvd_path,
                "output_file": ctx.output_file,
            });
            let _ = crate::utils::append_audit_log_entry(audit_path, "RipCompleted", &details);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_audit_log_post_processor_none() {
        let processor = AuditLogPostProcessor;
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        assert!(processor.process(&ctx).is_ok());
    }

    #[test]
    fn test_audit_log_post_processor_some() {
        let processor = AuditLogPostProcessor;
        let temp = std::env::temp_dir().join(format!("audit_test_{}.jsonl", std::process::id()));
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "The Matrix",
            PathBuf::from("D:\\"),
        );
        ctx.audit_log_path = Some(temp.clone());
        ctx.year = Some(1999);

        assert!(processor.process(&ctx).is_ok());
        assert!(temp.exists());
        let _ = std::fs::remove_file(temp);
    }
}
