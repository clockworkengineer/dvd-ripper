//! Interface Segregation Principle (ISP/SOLID): Segregated option value objects replacing monolithic `Args` dependencies.

use serde::{Deserialize, Serialize};

/// Segregated video codec and rate-control specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoEncodingSpec {
    pub codec: String,
    pub profile: String,
    pub preset: String,
    pub crf: Option<u8>,
    pub target_size_mb: Option<u32>,
    pub hwaccel: String,
    pub hwaccel_fallback: bool,
    pub tonemap: Option<String>,
}

impl Default for VideoEncodingSpec {
    fn default() -> Self {
        Self {
            codec: "h264".to_string(),
            profile: "standard".to_string(),
            preset: "slow".to_string(),
            crf: None,
            target_size_mb: None,
            hwaccel: "none".to_string(),
            hwaccel_fallback: true,
            tonemap: None,
        }
    }
}

/// Segregated video filter pipeline specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VideoFilterSpec {
    pub deinterlace: bool,
    pub deinterlace_algo: Option<String>,
    pub denoise: bool,
    pub auto_crop: bool,
    pub detelecine: bool,
}

/// Segregated audio stream selection and loudness normalization specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AudioEncodingSpec {
    pub all_audio: bool,
    pub normalize_audio: bool,
    pub norm_target: Option<i32>,
    pub dual_audio: bool,
    pub audio_track: Option<u32>,
    pub audio_lang: Option<String>,
    pub auto_audio_pref: Option<String>,
    pub audio_title: Option<String>,
    pub audio_downmix: Option<String>,
}

/// Segregated subtitle extraction, burn-in, and OCR specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SubtitleSpec {
    pub subtitles: bool,
    pub sub_lang: Option<String>,
    pub sub_format: Option<String>,
    pub sub_forced_only: bool,
    pub sub_default: bool,
    pub sub_burnin: bool,
    pub sub_external_srt: bool,
    pub ocr: bool,
    pub ocr_lang: String,
}

/// Segregated optical disc title, chapter, angle, and error-resilience specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiscSelectionSpec {
    pub title_number: u32,
    pub start_chapter: Option<u32>,
    pub end_chapter: Option<u32>,
    pub split_chapters: bool,
    pub angle: Option<u32>,
    pub rip_extras: bool,
    pub sector_retries: u32,
}

/// Segregated output file and container naming specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OutputNamingSpec {
    pub out_dir: String,
    pub explicit_output: Option<String>,
    pub mkv: bool,
    pub iso: bool,
    pub no_overwrite: bool,
}

impl From<&crate::cli::Args> for VideoEncodingSpec {
    fn from(args: &crate::cli::Args) -> Self {
        Self {
            codec: args.codec.clone(),
            profile: args.profile.clone(),
            preset: args.preset.clone(),
            crf: args.crf,
            target_size_mb: args.target_size_mb,
            hwaccel: args.hwaccel.clone(),
            hwaccel_fallback: args.hwaccel_fallback,
            tonemap: args.tonemap.clone(),
        }
    }
}

impl From<&crate::cli::Args> for VideoFilterSpec {
    fn from(args: &crate::cli::Args) -> Self {
        Self {
            deinterlace: args.deinterlace,
            deinterlace_algo: args.deinterlace_algo.clone(),
            denoise: args.denoise,
            auto_crop: args.auto_crop,
            detelecine: args.detelecine,
        }
    }
}

impl From<&crate::cli::Args> for AudioEncodingSpec {
    fn from(args: &crate::cli::Args) -> Self {
        Self {
            all_audio: args.all_audio,
            normalize_audio: args.normalize_audio,
            norm_target: args.norm_target,
            dual_audio: args.dual_audio,
            audio_track: args.audio_track,
            audio_lang: args.audio_lang.clone(),
            auto_audio_pref: args.auto_audio_pref.clone(),
            audio_title: args.audio_title.clone(),
            audio_downmix: args.audio_downmix.clone(),
        }
    }
}

impl From<&crate::cli::Args> for SubtitleSpec {
    fn from(args: &crate::cli::Args) -> Self {
        Self {
            subtitles: args.subtitles,
            sub_lang: args.sub_lang.clone(),
            sub_format: args.sub_format.clone(),
            sub_forced_only: args.sub_forced_only,
            sub_default: args.sub_default,
            sub_burnin: args.sub_burnin,
            sub_external_srt: args.sub_external_srt,
            ocr: args.ocr,
            ocr_lang: args.ocr_lang.clone(),
        }
    }
}

impl From<&crate::cli::Args> for DiscSelectionSpec {
    fn from(args: &crate::cli::Args) -> Self {
        Self {
            title_number: args.title,
            start_chapter: args.start_chapter,
            end_chapter: args.end_chapter,
            split_chapters: args.split_chapters,
            angle: args.angle,
            rip_extras: args.rip_extras,
            sector_retries: args.sector_retries,
        }
    }
}

impl From<&crate::cli::Args> for OutputNamingSpec {
    fn from(args: &crate::cli::Args) -> Self {
        Self {
            out_dir: args.out_dir.clone(),
            explicit_output: args.output.clone(),
            mkv: args.mkv,
            iso: args.iso,
            no_overwrite: args.no_overwrite,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segregated_specs_from_args() {
        let mut args = crate::cli::Args::default();
        args.codec = "hevc".to_string();
        args.crf = Some(20);
        args.auto_crop = true;
        args.detelecine = true;
        args.all_audio = true;
        args.subtitles = true;
        args.start_chapter = Some(2);
        args.end_chapter = Some(5);
        args.mkv = true;

        let video = VideoEncodingSpec::from(&args);
        assert_eq!(video.codec, "hevc");
        assert_eq!(video.crf, Some(20));

        let filters = VideoFilterSpec::from(&args);
        assert!(filters.auto_crop);
        assert!(filters.detelecine);

        let audio = AudioEncodingSpec::from(&args);
        assert!(audio.all_audio);

        let subs = SubtitleSpec::from(&args);
        assert!(subs.subtitles);

        let disc = DiscSelectionSpec::from(&args);
        assert_eq!(disc.start_chapter, Some(2));
        assert_eq!(disc.end_chapter, Some(5));

        let naming = OutputNamingSpec::from(&args);
        assert!(naming.mkv);
    }
}
