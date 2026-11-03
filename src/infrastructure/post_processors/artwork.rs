use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that saves movie / series cover poster artworks next to the target video file.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArtworkPostProcessor;

impl PostProcessor for ArtworkPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if let (Some(bytes), Some(out_file)) = (ctx.poster_bytes.as_ref(), ctx.output_file.as_ref())
        {
            let _ = crate::utils::save_cover_artworks(out_file, bytes);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_artwork_post_processor_no_crash_on_none() {
        let processor = ArtworkPostProcessor;
        let ctx = PostProcessContext::new(None, "Movie", PathBuf::from("D:\\"));
        assert!(processor.process(&ctx).is_ok());
    }
}
