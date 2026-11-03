use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that generates Kodi, Jellyfin, and Plex compatible .nfo XML metadata files.
#[derive(Debug, Default, Clone, Copy)]
pub struct NfoPostProcessor;

impl PostProcessor for NfoPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if ctx.nfo_enabled
            && let Some(ref out_file) = ctx.output_file
        {
            let _ = crate::utils::generate_nfo_file(
                out_file,
                &ctx.title,
                ctx.year,
                None,
                None,
                None,
                &ctx.media_type,
                ctx.nfo_tags.as_deref(),
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
    fn test_nfo_post_processor_disabled() {
        let processor = NfoPostProcessor;
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        ctx.nfo_enabled = false;
        assert!(processor.process(&ctx).is_ok());
    }

    #[test]
    fn test_nfo_post_processor_enabled() {
        let processor = NfoPostProcessor;
        let temp_dir = std::env::temp_dir().join(format!("nfo_proc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let out_file = temp_dir.join("film.mp4");

        let mut ctx = PostProcessContext::new(
            Some(out_file),
            "Aliens",
            PathBuf::from("D:\\"),
        );
        ctx.nfo_enabled = true;
        ctx.year = Some(1986);
        ctx.nfo_tags = Some("Sci-Fi".to_string());

        assert!(processor.process(&ctx).is_ok());

        let nfo = temp_dir.join("film.nfo");
        assert!(nfo.exists());
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
