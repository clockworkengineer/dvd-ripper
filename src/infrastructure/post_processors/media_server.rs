use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;

/// Post-processor that triggers library scans on Jellyfin, Plex, and Emby media servers.
#[derive(Debug, Default, Clone, Copy)]
pub struct MediaServerScanPostProcessor;

impl PostProcessor for MediaServerScanPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if ctx.media_servers_enabled {
            crate::infrastructure::notifications::media_servers::trigger_media_servers(
                ctx.plex_url.as_deref().zip(ctx.plex_token.as_deref()),
                ctx.jellyfin_url.as_deref().zip(ctx.jellyfin_key.as_deref()),
                ctx.emby_url.as_deref().zip(ctx.emby_key.as_deref()),
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
    fn test_media_server_post_processor_disabled() {
        let processor = MediaServerScanPostProcessor;
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        assert!(processor.process(&ctx).is_ok());
    }

    #[test]
    fn test_media_server_post_processor_enabled() {
        let processor = MediaServerScanPostProcessor;
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        ctx.media_servers_enabled = true;
        ctx.plex_url = Some("http://127.0.0.1:32400".to_string());
        ctx.plex_token = Some("plextoken".to_string());
        ctx.jellyfin_url = Some("http://127.0.0.1:8096".to_string());
        ctx.jellyfin_key = Some("jfkey".to_string());
        ctx.emby_url = Some("http://127.0.0.1:8096".to_string());
        ctx.emby_key = Some("embykey".to_string());

        assert!(processor.process(&ctx).is_ok());
    }
}
