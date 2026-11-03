use crate::domain::ports::optical_drive::TrayController;
use crate::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use anyhow::Result;
use std::sync::Arc;

/// Post-processor that manages hardware optical drive tray ejection, auto-close timers, and spindown.
pub struct TrayEjectPostProcessor {
    tray_controller: Option<Arc<dyn TrayController>>,
}

impl TrayEjectPostProcessor {
    pub fn new(tray_controller: Option<Arc<dyn TrayController>>) -> Self {
        Self { tray_controller }
    }
}

impl PostProcessor for TrayEjectPostProcessor {
    fn process(&self, ctx: &PostProcessContext) -> Result<()> {
        if !ctx.no_eject {
            if let Some(ref controller) = self.tray_controller {
                let _ = controller.eject_tray(&ctx.dvd_path);
            } else {
                let _ = crate::dvd::eject_disc(&ctx.dvd_path.to_string_lossy());
            }

            if let Some(secs) = ctx.eject_autoclose_secs {
                std::thread::sleep(std::time::Duration::from_secs(secs));
                if let Some(ref controller) = self.tray_controller {
                    let _ = controller.close_tray(&ctx.dvd_path);
                } else {
                    let _ = crate::dvd::close_disc_tray(&ctx.dvd_path.to_string_lossy());
                }
            }
        } else if ctx.spindown {
            if let Some(ref controller) = self.tray_controller {
                let _ = controller.spindown(&ctx.dvd_path);
            } else {
                let _ = crate::dvd::spindown_drive(&ctx.dvd_path.to_string_lossy());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockTrayController {
        ejected: AtomicBool,
        closed: AtomicBool,
        spundown: AtomicBool,
    }

    impl TrayController for MockTrayController {
        fn eject_tray(&self, _drive_path: &Path) -> Result<()> {
            self.ejected.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn close_tray(&self, _drive_path: &Path) -> Result<()> {
            self.closed.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn spindown(&self, _drive_path: &Path) -> Result<()> {
            self.spundown.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_tray_eject_post_processor_ejects() {
        let mock = Arc::new(MockTrayController {
            ejected: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            spundown: AtomicBool::new(false),
        });
        let processor = TrayEjectPostProcessor::new(Some(mock.clone()));
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        ctx.no_eject = false;
        assert!(processor.process(&ctx).is_ok());
        assert!(mock.ejected.load(Ordering::SeqCst));
        assert!(!mock.spundown.load(Ordering::SeqCst));
    }

    #[test]
    fn test_tray_eject_post_processor_spindown() {
        let mock = Arc::new(MockTrayController {
            ejected: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            spundown: AtomicBool::new(false),
        });
        let processor = TrayEjectPostProcessor::new(Some(mock.clone()));
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        ctx.no_eject = true;
        ctx.spindown = true;
        assert!(processor.process(&ctx).is_ok());
        assert!(!mock.ejected.load(Ordering::SeqCst));
        assert!(mock.spundown.load(Ordering::SeqCst));
    }

    #[test]
    fn test_tray_eject_post_processor_autoclose_and_fallback() {
        let mock = Arc::new(MockTrayController {
            ejected: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            spundown: AtomicBool::new(false),
        });
        let processor = TrayEjectPostProcessor::new(Some(mock.clone()));
        let mut ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Movie",
            PathBuf::from("D:\\"),
        );
        ctx.no_eject = false;
        ctx.eject_autoclose_secs = Some(0); // 0s sleep for fast tests

        assert!(processor.process(&ctx).is_ok());
        assert!(mock.ejected.load(Ordering::SeqCst));
        assert!(mock.closed.load(Ordering::SeqCst));

        // When tray_controller is None (direct fallback)
        let processor_none = TrayEjectPostProcessor::new(None);
        assert!(processor_none.process(&ctx).is_ok());

        let mut ctx_spindown = ctx.clone();
        ctx_spindown.no_eject = true;
        ctx_spindown.spindown = true;
        assert!(processor_none.process(&ctx_spindown).is_ok());
    }
}
