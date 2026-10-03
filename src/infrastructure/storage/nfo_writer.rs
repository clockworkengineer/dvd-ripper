use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::domain::entities::media::FilmMetadata;

/// Resolves sidecar `.nfo` metadata file path from video output path.
pub fn resolve_nfo_path(video_path: &Path) -> PathBuf {
    let parent = video_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = video_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "output".to_string());
    parent.join(format!("{}.nfo", stem))
}

/// Generates a Kodi/Jellyfin/Plex compatible XML `.nfo` sidecar file alongside the transcoded video.
pub fn generate_nfo_file(
    output_file: &Path,
    metadata: &FilmMetadata,
    media_type: &str,
) -> Result<()> {
    crate::utils::generate_nfo_file(
        output_file,
        &metadata.title,
        metadata.year,
        metadata.plot.as_deref(),
        metadata.rating.as_deref(),
        metadata.director.as_deref(),
        media_type,
        metadata.genre.as_deref(),
    )
}
