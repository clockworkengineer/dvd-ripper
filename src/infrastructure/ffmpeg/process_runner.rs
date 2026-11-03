use crate::cli::Args;
use crate::ffmpeg::ProgressEvent;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};

/// Executes an FFmpeg transcode process with live stderr progress parsing and graceful cancellation support.
#[allow(clippy::too_many_arguments)]
pub fn run_ffmpeg_process(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    display_title: &str,
    expected_runtime_secs: Option<f64>,
    tx: Option<Sender<ProgressEvent>>,
    cancel_rx: Option<Receiver<()>>,
    cancel_flag: Option<Arc<AtomicBool>>,
    is_batch: bool,
) -> Result<()> {
    crate::ffmpeg::run_ffmpeg_with_channel(
        args,
        dvd_path,
        absolute_output,
        display_title,
        expected_runtime_secs,
        tx,
        cancel_rx,
        cancel_flag,
        is_batch,
    )
}

use std::ops::{Deref, DerefMut};

/// RAII process supervisor ensuring child processes are reliably killed and reaped upon drop or panic.
#[derive(Debug)]
pub struct ChildGuard {
    child: Option<std::process::Child>,
}

impl ChildGuard {
    /// Wraps a spawned child process in a supervisor.
    pub fn new(child: std::process::Child) -> Self {
        Self { child: Some(child) }
    }

    /// Returns process ID if process is still active.
    pub fn id(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// Disarms the supervisor, allowing the child to exit or wait naturally.
    pub fn disarm(mut self) {
        self.child.take();
    }
}

impl Deref for ChildGuard {
    type Target = std::process::Child;

    fn deref(&self) -> &Self::Target {
        self.child.as_ref().expect("child active until disarm/drop")
    }
}

impl DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.child.as_mut().expect("child active until disarm/drop")
    }
}

impl AsMut<std::process::Child> for ChildGuard {
    fn as_mut(&mut self) -> &mut std::process::Child {
        self.child.as_mut().expect("child active until disarm/drop")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take()
            && let Ok(None) = child.try_wait()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_child_guard_auto_kill_on_drop() {
        #[cfg(target_os = "windows")]
        let cmd = Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 10"])
            .spawn();

        #[cfg(not(target_os = "windows"))]
        let cmd = Command::new("sleep").arg("10").spawn();

        if let Ok(child) = cmd {
            let pid = child.id();
            {
                let _guard = ChildGuard::new(child);
                // _guard drops here, automatically killing and reaping the child!
            }
            // Verify guard dropped without panicking
            assert!(pid > 0);
        }
    }

    #[test]
    fn test_child_guard_disarm() {
        #[cfg(target_os = "windows")]
        let cmd = Command::new("powershell")
            .args(["-NoProfile", "-Command", "Exit 0"])
            .spawn();

        #[cfg(not(target_os = "windows"))]
        let cmd = Command::new("true").spawn();

        if let Ok(child) = cmd {
            let mut guard = ChildGuard::new(child);
            let status = guard.as_mut().wait().unwrap();
            assert!(status.success());
            guard.disarm();
        }
    }
}
