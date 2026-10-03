use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use anyhow::{anyhow, Result};

use crate::domain::entities::disc::TvEpisodeInfo;
use crate::domain::entities::media::FilmMetadata;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::{ProgressObserver, Transcoder};
use super::rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};

/// Request parameter object for executing a batch TV season ripping operation.
#[derive(Debug, Clone)]
pub struct RipTvBatchRequest {
    pub source_dvd: PathBuf,
    pub series_title: String,
    pub season_number: u32,
    pub start_episode: u32,
    pub output_dir: PathBuf,
    pub episodes: Vec<TvEpisodeInfo>,
    pub metadata: Option<FilmMetadata>,
    pub min_free_gb: u64,
}

/// Use case that orchestrates batch extraction of multiple TV episodes from an optical disc.
pub struct RipTvBatchUseCase {
    single_title_use_case: RipSingleTitleUseCase,
    notifier: Arc<dyn Notifier>,
}

impl RipTvBatchUseCase {
    pub fn new(
        transcoder: Arc<dyn Transcoder>,
        notifier: Arc<dyn Notifier>,
        disk_guard: Arc<dyn DiskSpaceGuard>,
        history_repo: Arc<dyn HistoryRepository>,
    ) -> Self {
        Self {
            single_title_use_case: RipSingleTitleUseCase::new(
                transcoder,
                notifier.clone(),
                disk_guard,
                history_repo,
            ),
            notifier,
        }
    }

    pub fn execute(
        &self,
        request: &RipTvBatchRequest,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<usize> {
        let mut completed_count = 0;

        for ep in &request.episodes {
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    let _ = self.notifier.send(&NotificationEvent::RipCancelled {
                        title: &request.series_title,
                    });
                    return Err(anyhow!("TV Batch ripping cancelled by user."));
                }
            }

            let ep_filename = format!(
                "{} - S{:02}E{:02}.mp4",
                request.series_title, request.season_number, ep.episode_num
            );
            let ep_output_path = request.output_dir.join(&ep_filename);

            let single_req = RipSingleTitleRequest {
                source_dvd: request.source_dvd.clone(),
                title_number: ep.title_num,
                output_path: ep_output_path,
                display_title: format!(
                    "{} - S{:02}E{:02}",
                    request.series_title, request.season_number, ep.episode_num
                ),
                expected_duration_secs: Some(ep.duration_secs),
                metadata: request.metadata.clone(),
                min_free_gb: request.min_free_gb,
            };

            self.single_title_use_case.execute(&single_req, observer, cancel_flag.clone())?;
            completed_count += 1;
        }

        Ok(completed_count)
    }
}
