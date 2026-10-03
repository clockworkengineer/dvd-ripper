//! # Application Use Case Layer
//!
//! Orchestrates the flow of data to and from domain entities and directs those entities
//! to apply their business rules.
//!
//! Use cases in this layer interact exclusively with domain port traits ([`crate::domain::ports`]),
//! keeping the application logic decoupled from concrete operating system or FFmpeg implementations.
//!
//! Submodules:
//! - [`probe_disc`]: [`ProbeDiscUseCase`] for querying disc metadata and analyzing title tracks.
//! - [`rip_single_title`]: [`RipSingleTitleUseCase`] for single movie ripping pipelines.
//! - [`rip_tv_batch`]: [`RipTvBatchUseCase`] for batch ripping multi-episode television series.
//! - [`drive_pool`]: [`DrivePool`] for managing concurrent multi-drive ripping operations.

pub mod drive_pool;
pub mod probe_disc;
pub mod rip_single_title;
pub mod rip_tv_batch;

pub use drive_pool::DrivePool;
pub use probe_disc::{DiscProbeResult, ProbeDiscUseCase};
pub use rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};
pub use rip_tv_batch::{RipTvBatchRequest, RipTvBatchUseCase};

#[cfg(test)]
mod tests;
