use crate::cli::Args;
use crate::domain::ports::transcoder::{DefaultVideoCodecResolver, VideoCodecResolver};
use std::path::Path;
use std::process::Command;

/// Builder pattern for assembling configured FFmpeg child process commands with pluggable codec strategies.
pub struct FfmpegCommandBuilder<'a> {
    args: &'a Args,
    dvd_path: &'a Path,
    output_path: &'a Path,
    title_num: u32,
    codec_resolver: &'a dyn VideoCodecResolver,
}

impl<'a> FfmpegCommandBuilder<'a> {
    pub fn new(args: &'a Args, dvd_path: &'a Path, output_path: &'a Path, title_num: u32) -> Self {
        Self {
            args,
            dvd_path,
            output_path,
            title_num,
            codec_resolver: &DefaultVideoCodecResolver,
        }
    }

    pub fn with_codec_resolver(mut self, resolver: &'a dyn VideoCodecResolver) -> Self {
        self.codec_resolver = resolver;
        self
    }

    pub fn build(self) -> Command {
        crate::ffmpeg::build_ffmpeg_command(
            self.args,
            self.dvd_path,
            self.output_path,
            self.title_num,
        )
    }
}

/// Builds an FFmpeg Command configured with parameters for the specified input title and target output.
pub fn build_ffmpeg_command(
    args: &Args,
    dvd_path: &Path,
    output_path: &Path,
    title_num: u32,
) -> Command {
    FfmpegCommandBuilder::new(args, dvd_path, output_path, title_num).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CustomCodecResolver;
    impl VideoCodecResolver for CustomCodecResolver {
        fn resolve(&self, _codec: &str) -> &'static str {
            "custom_codec_v1"
        }
    }

    #[test]
    fn test_ffmpeg_command_builder_construction() {
        let args = Args::default();
        let builder = FfmpegCommandBuilder::new(&args, Path::new("D:\\"), Path::new("out.mp4"), 1)
            .with_codec_resolver(&CustomCodecResolver);

        let cmd = builder.build();
        let program = cmd.get_program();
        assert_eq!(program, "ffmpeg");
    }
}
