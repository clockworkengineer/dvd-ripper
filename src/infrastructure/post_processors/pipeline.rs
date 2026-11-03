use crate::domain::ports::optical_drive::TrayController;
use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;
use std::sync::Arc;

/// Open/Closed Principle (OCP/SOLID): Composite post-processing pipeline executing registered processors.
pub struct PostProcessingPipeline {
    processors: Vec<Box<dyn PostProcessor>>,
}

impl PostProcessingPipeline {
    pub fn new(processors: Vec<Box<dyn PostProcessor>>) -> Self {
        Self { processors }
    }

    pub fn empty() -> Self {
        Self {
            processors: Vec::new(),
        }
    }

    pub fn add_processor(&mut self, processor: Box<dyn PostProcessor>) {
        self.processors.push(processor);
    }

    /// Builds the standard production post-processing pipeline.
    pub fn build_production(tray_controller: Option<Arc<dyn TrayController>>) -> Self {
        let processors: Vec<Box<dyn PostProcessor>> = vec![
            Box::new(super::artwork::ArtworkPostProcessor),
            Box::new(super::nfo::NfoPostProcessor),
            Box::new(super::checksum::ChecksumPostProcessor),
            Box::new(super::audit_log::AuditLogPostProcessor),
            Box::new(super::history_purge::HistoryPurgePostProcessor),
            Box::new(super::script_hook::ScriptHookPostProcessor),
            Box::new(super::media_server::MediaServerScanPostProcessor),
            Box::new(super::tray_eject::TrayEjectPostProcessor::new(
                tray_controller,
            )),
        ];
        Self::new(processors)
    }
}

impl PostProcessor for PostProcessingPipeline {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        for processor in &self.processors {
            if let Err(e) = processor.process(ctx) {
                eprintln!("Warning: Post-processor execution error: {}", e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingPostProcessor {
        counter: Arc<AtomicUsize>,
    }

    impl PostProcessor for CountingPostProcessor {
        fn process(&self, _ctx: &PostProcessContext) -> Result<()> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_pipeline_executes_all_processors() {
        let counter = Arc::new(AtomicUsize::new(0));
        let p1 = Box::new(CountingPostProcessor {
            counter: counter.clone(),
        });
        let p2 = Box::new(CountingPostProcessor {
            counter: counter.clone(),
        });

        let pipeline = PostProcessingPipeline::new(vec![p1, p2]);
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("video.mp4")),
            "Test",
            PathBuf::from("D:\\"),
        );

        pipeline.process(&ctx).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
