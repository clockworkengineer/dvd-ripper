//! Pure domain naming conventions for Plex/Jellyfin/Emby media library organization (SRP).

use crate::domain::entities::options::OutputNamingSpec;
use std::path::{Path, PathBuf};

/// Formats a sanitized media folder/file stem `<Title> (<Year>)` or `<Title>`.
pub fn format_media_stem(title: Option<&str>, year: Option<u32>, fallback: &str) -> String {
    let raw = title
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback);
    let clean = crate::utils::sanitize_filename(raw);
    let base = if clean.is_empty() {
        fallback.to_string()
    } else {
        clean
    };
    match year {
        Some(y) if y >= 1888 && !base.contains(&format!("({})", y)) => format!("{} ({})", base, y),
        _ => base,
    }
}

/// Resolves the output file extension (`iso`, `mkv`, or `mp4`) from a segregated [`OutputNamingSpec`].
pub fn resolve_container_extension(spec: &OutputNamingSpec) -> &'static str {
    if spec.iso {
        "iso"
    } else if spec.mkv {
        "mkv"
    } else {
        "mp4"
    }
}

/// Resolves a movie output path purely from [`OutputNamingSpec`], title, and optional release year.
pub fn resolve_movie_output_path(
    spec: &OutputNamingSpec,
    title: Option<&str>,
    year: Option<u32>,
) -> PathBuf {
    if let Some(ref explicit) = spec.explicit_output
        && !explicit.trim().is_empty()
    {
        return PathBuf::from(explicit.trim());
    }
    let ext = resolve_container_extension(spec);
    let stem = format_media_stem(title, year, "DVD_Rip");
    Path::new(&spec.out_dir).join(format!("{}.{}", stem, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_naming_conventions() {
        let spec = OutputNamingSpec {
            out_dir: "/movies".to_string(),
            explicit_output: None,
            mkv: true,
            iso: false,
            no_overwrite: false,
        };
        let p = resolve_movie_output_path(&spec, Some("The Matrix"), Some(1999));
        assert!(p.to_string_lossy().ends_with("The Matrix (1999).mkv"));
    }
}

