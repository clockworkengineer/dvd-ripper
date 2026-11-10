//! Segregated domain ports for batch job queue persistence and multi-disc TV box-set tracking (ISP/DIP).

use crate::queue::{BoxSetRecord, JobItem};

/// Interface Segregation & Dependency Inversion (ISP/DIP): Port for batch job queue persistence.
pub trait JobQueueRepository: Send + Sync {
    fn name(&self) -> &str;
    fn add_job(&self, title: &str, media_type: &str, drive: &str) -> String;
    fn list_jobs(&self) -> Vec<JobItem>;
    fn remove_job(&self, id: &str) -> bool;
    fn next_ready_job(&self) -> Option<JobItem>;
    fn update_job_status(&self, id: &str, status: &str, progress: f64) -> bool;
    fn move_job_up(&self, id: &str) -> bool;
    fn move_job_down(&self, id: &str) -> bool;
    fn clear_queue(&self);
}

/// Interface Segregation & Dependency Inversion (ISP/DIP): Port for multi-disc TV box-set episode tracking.
pub trait BoxSetRepository: Send + Sync {
    fn get_next_episode(&self, show_name: &str, season: u32) -> u32;
    fn record_episodes_ripped(&self, show_name: &str, season: u32, count: u32) -> u32;
    fn reset_tracker(&self, show_name: &str, season: u32) -> bool;
    fn list_boxsets(&self) -> Vec<BoxSetRecord>;
}

