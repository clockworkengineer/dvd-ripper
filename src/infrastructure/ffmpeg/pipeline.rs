//! Open/Closed Principle (OCP/SOLID): Composable FFmpeg argument and filter contributors.

use crate::domain::entities::options::{
    AudioEncodingSpec, DiscSelectionSpec, VideoEncodingSpec, VideoFilterSpec,
};

/// Context passed to each [`FfmpegArgContributor`] stage during command assembly.
#[derive(Debug, Clone)]
pub struct TranscodePipelineContext {
    pub title_num: u32,
    pub disc: DiscSelectionSpec,
    pub video: VideoEncodingSpec,
    pub filters: VideoFilterSpec,
    pub audio: AudioEncodingSpec,
    pub detected_crop: Option<String>,
}

/// Open/Closed (OCP/SOLID) strategy trait for modular FFmpeg argument & filter chain contribution.
/// New filters, pre-input demuxer flags, or post-output muxer flags can be added by implementing
/// this trait and registering the contributor without modifying existing builder logic.
pub trait FfmpegArgContributor: Send + Sync {
    /// Contributes pre-input demuxer flags (e.g., `-title`, `-chapter_start`, `-angle`).
    fn contribute_input_args(&self, _ctx: &TranscodePipelineContext, _args: &mut Vec<String>) {}

    /// Contributes video filter expressions (`-vf`) to the comma-separated filtergraph.
    fn contribute_video_filters(
        &self,
        _ctx: &TranscodePipelineContext,
        _filters: &mut Vec<String>,
    ) {
    }

    /// Contributes audio filter expressions (`-af`) to the comma-separated filtergraph.
    fn contribute_audio_filters(
        &self,
        _ctx: &TranscodePipelineContext,
        _filters: &mut Vec<String>,
    ) {
    }

    /// Contributes post-input encoding/muxing flags.
    fn contribute_output_args(&self, _ctx: &TranscodePipelineContext, _args: &mut Vec<String>) {}
}

/// Contributes DVD chapter range (`-chapter_start`, `-chapter_end`) and multi-angle (`-angle`) flags.
#[derive(Debug, Default)]
pub struct ChapterAngleContributor;

impl FfmpegArgContributor for ChapterAngleContributor {
    fn contribute_input_args(&self, ctx: &TranscodePipelineContext, args: &mut Vec<String>) {
        if let Some(ch_start) = ctx.disc.start_chapter
            && ch_start > 0
        {
            args.push("-chapter_start".to_string());
            args.push(ch_start.to_string());
        }
        if let Some(ch_end) = ctx.disc.end_chapter
            && ch_end > 0
        {
            args.push("-chapter_end".to_string());
            args.push(ch_end.to_string());
        }
        if let Some(angle) = ctx.disc.angle
            && angle > 0
        {
            args.push("-angle".to_string());
            args.push(angle.to_string());
        }
    }
}

/// Contributes inverse telecine (`fieldmatch,yadif,decimate`), deinterlacing (`bwdif`/`yadif`),
/// auto-cropping (`crop=w:h:x:y`), and spatial-temporal denoising (`hqdn3d`) filters.
#[derive(Debug, Default)]
pub struct VideoFilterContributor;

impl FfmpegArgContributor for VideoFilterContributor {
    fn contribute_video_filters(&self, ctx: &TranscodePipelineContext, filters: &mut Vec<String>) {
        if ctx.filters.detelecine {
            filters.push("fieldmatch=order=auto:mode=pc:combmatch=full,yadif=deint=interlaced,decimate=cycle=5".to_string());
        } else if ctx.filters.deinterlace {
            let algo = ctx.filters.deinterlace_algo.as_deref().unwrap_or("bwdif");
            if algo.eq_ignore_ascii_case("yadif") {
                filters.push("yadif=0:-1:0".to_string());
            } else {
                filters.push("bwdif=mode=send_frame:parity=auto:deint=all".to_string());
            }
        }
        if let Some(ref crop) = ctx.detected_crop
            && !crop.is_empty()
        {
            filters.push(format!("crop={}", crop));
        }
        if ctx.filters.denoise {
            filters.push("hqdn3d=1.5:1.5:6:6".to_string());
        }
    }
}

/// Contributes EBU R128 loudness normalization (`loudnorm`) audio filters.
#[derive(Debug, Default)]
pub struct AudioFilterContributor;

impl FfmpegArgContributor for AudioFilterContributor {
    fn contribute_audio_filters(&self, ctx: &TranscodePipelineContext, filters: &mut Vec<String>) {
        if ctx.audio.normalize_audio {
            let target = ctx.audio.norm_target.unwrap_or(-23).clamp(-70, -5);
            filters.push(format!("loudnorm=I={}:TP=-1.5:LRA=11", target));
        }
    }
}

/// Open/Closed composite pipeline executing registered [`FfmpegArgContributor`] stages in sequence.
#[derive(Default)]
pub struct FfmpegArgPipeline {
    contributors: Vec<Box<dyn FfmpegArgContributor>>,
}

impl FfmpegArgPipeline {
    /// Creates an empty [`FfmpegArgPipeline`].
    pub fn new() -> Self {
        Self {
            contributors: Vec::new(),
        }
    }

    /// Creates the standard production pipeline with chapter/angle, video filter, and audio filter contributors.
    pub fn build_default() -> Self {
        let mut pipeline = Self::new();
        pipeline.register(Box::new(ChapterAngleContributor));
        pipeline.register(Box::new(VideoFilterContributor));
        pipeline.register(Box::new(AudioFilterContributor));
        pipeline
    }

    /// Registers a new [`FfmpegArgContributor`] stage without modifying existing contributors (OCP).
    pub fn register(&mut self, contributor: Box<dyn FfmpegArgContributor>) {
        self.contributors.push(contributor);
    }

    /// Collects all pre-input arguments from registered contributors.
    pub fn collect_input_args(&self, ctx: &TranscodePipelineContext) -> Vec<String> {
        let mut args = Vec::new();
        for c in &self.contributors {
            c.contribute_input_args(ctx, &mut args);
        }
        args
    }

    /// Collects all video filter expressions (`-vf`) from registered contributors.
    pub fn collect_video_filters(&self, ctx: &TranscodePipelineContext) -> Vec<String> {
        let mut filters = Vec::new();
        for c in &self.contributors {
            c.contribute_video_filters(ctx, &mut filters);
        }
        filters
    }

    /// Collects all audio filter expressions (`-af`) from registered contributors.
    pub fn collect_audio_filters(&self, ctx: &TranscodePipelineContext) -> Vec<String> {
        let mut filters = Vec::new();
        for c in &self.contributors {
            c.contribute_audio_filters(ctx, &mut filters);
        }
        filters
    }

    /// Collects all post-input output arguments from registered contributors.
    pub fn collect_output_args(&self, ctx: &TranscodePipelineContext) -> Vec<String> {
        let mut args = Vec::new();
        for c in &self.contributors {
            c.contribute_output_args(ctx, &mut args);
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CustomWatermarkContributor;
    impl FfmpegArgContributor for CustomWatermarkContributor {
        fn contribute_video_filters(
            &self,
            _ctx: &TranscodePipelineContext,
            filters: &mut Vec<String>,
        ) {
            filters.push("scale=1920:1080".to_string());
        }
    }

    #[test]
    fn test_ocp_ffmpeg_arg_pipeline_extensibility() {
        let mut pipeline = FfmpegArgPipeline::build_default();
        pipeline.register(Box::new(CustomWatermarkContributor));

        let ctx = TranscodePipelineContext {
            title_num: 1,
            disc: DiscSelectionSpec {
                title_number: 1,
                start_chapter: Some(2),
                end_chapter: Some(4),
                split_chapters: false,
                angle: Some(2),
                rip_extras: false,
                sector_retries: 2,
            },
            video: VideoEncodingSpec::default(),
            filters: VideoFilterSpec {
                deinterlace: false,
                deinterlace_algo: None,
                denoise: true,
                auto_crop: true,
                detelecine: true,
            },
            audio: AudioEncodingSpec {
                normalize_audio: true,
                norm_target: Some(-16),
                ..Default::default()
            },
            detected_crop: Some("720:360:0:60".to_string()),
        };

        let in_args = pipeline.collect_input_args(&ctx);
        assert!(in_args.contains(&"-chapter_start".to_string()));
        assert!(in_args.contains(&"-angle".to_string()));

        let v_filters = pipeline.collect_video_filters(&ctx);
        assert_eq!(v_filters.len(), 4);
        assert!(v_filters[0].contains("fieldmatch"));
        assert_eq!(v_filters[1], "crop=720:360:0:60");
        assert!(v_filters[2].contains("hqdn3d"));
        assert_eq!(v_filters[3], "scale=1920:1080");

        let a_filters = pipeline.collect_audio_filters(&ctx);
        assert_eq!(a_filters, vec!["loudnorm=I=-16:TP=-1.5:LRA=11"]);
    }
}

