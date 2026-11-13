//! Shared application lifecycle coordinator for recording job completion and notifying observers (DRY).

use crate::domain::entities::error::DvdError;
use crate::domain::entities::job::RipRecord;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::storage::HistoryRepository;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Context parameter object for recording rip or dump outcomes.
#[derive(Debug, Clone)]
pub struct RipOutcomeContext<'a> {
    pub display_title: &'a str,
    pub media_type: &'a str,
    pub output_path: &'a Path,
    pub volume_label: Option<String>,
    pub year: Option<u32>,
    pub duration_secs: f64,
    pub bytes_written: Option<u64>,
    pub codec: Option<String>,
}

/// Records history and dispatches notifications for use cases upon completion or failure.
pub fn record_and_notify_outcome<T>(
    result: Result<T>,
    ctx: RipOutcomeContext<'_>,
    history_repo: &dyn HistoryRepository,
    notifier: &dyn Notifier,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<T> {
    let out_str = ctx.output_path.to_string_lossy();
    match result {
        Ok(val) => {
            let record = RipRecord::with_details(
                ctx.display_title,
                ctx.media_type,
                &out_str,
                "Completed",
                ctx.volume_label,
                ctx.year,
                Some(ctx.duration_secs),
                ctx.bytes_written.or_else(|| {
                    std::fs::metadata(ctx.output_path).ok().map(|m| m.len())
                }),
                ctx.codec,
                None,
                None,
            );
            let _ = history_repo.append_record(&record, None);

            let _ = notifier.send(&NotificationEvent::RipSuccess {
                title: ctx.display_title,
                output_file: &out_str,
                duration_secs: ctx.duration_secs,
            });

            Ok(val)
        }
        Err(e) => {
            let is_cancelled = cancel_flag
                .is_some_and(|f| f.load(Ordering::SeqCst))
                || e.to_string().to_lowercase().contains("cancel");

            if is_cancelled {
                let record =
                    RipRecord::new(ctx.display_title, ctx.media_type, &out_str, "Cancelled");
                let _ = history_repo.append_record(&record, None);

                let _ = notifier.send(&NotificationEvent::RipCancelled {
                    title: ctx.display_title,
                });

                Err(DvdError::Cancelled.into())
            } else {
                let err_str = e.to_string();
                let record =
                    RipRecord::new(ctx.display_title, ctx.media_type, &out_str, "Failed");
                let _ = history_repo.append_record(&record, None);

                let _ = notifier.send(&NotificationEvent::RipFailed {
                    title: ctx.display_title,
                    error_message: &err_str,
                });

                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::testing::{InMemoryHistoryRepository, RecordingNotifier};

    #[test]
    fn test_record_and_notify_outcome_success() {
        let history = InMemoryHistoryRepository::new();
        let notifier = RecordingNotifier::new();
        let dummy_path = Path::new("test_out.mp4");

        let ctx = RipOutcomeContext {
            display_title: "Test Film",
            media_type: "Movie",
            output_path: dummy_path,
            volume_label: None,
            year: Some(2021),
            duration_secs: 100.0,
            bytes_written: Some(1024),
            codec: Some("h264".to_string()),
        };

        let res = record_and_notify_outcome(Ok(42), ctx, &history, &notifier, None);
        assert_eq!(res.unwrap(), 42);

        let records = history.load_history(None);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, "Completed");
        assert_eq!(records[0].title, "Test Film");

        assert_eq!(notifier.events().len(), 1);
    }

    #[test]
    fn test_record_and_notify_outcome_cancelled() {
        let history = InMemoryHistoryRepository::new();
        let notifier = RecordingNotifier::new();
        let dummy_path = Path::new("test_cancel.mp4");
        let flag = Arc::new(AtomicBool::new(true));

        let ctx = RipOutcomeContext {
            display_title: "Cancelled Film",
            media_type: "Movie",
            output_path: dummy_path,
            volume_label: None,
            year: None,
            duration_secs: 50.0,
            bytes_written: None,
            codec: None,
        };

        let res: Result<()> = record_and_notify_outcome(
            Err(anyhow::anyhow!("Operation cancelled")),
            ctx,
            &history,
            &notifier,
            Some(&flag),
        );
        assert!(res.is_err());

        let records = history.load_history(None);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, "Cancelled");
    }
}

