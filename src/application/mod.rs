//! # Application Use Case Layer
//!
//! Orchestrates the flow of data to and from domain entities and directs those entities
//! to apply their business rules.
//!
//! Use cases in this layer interact exclusively with domain port traits ([`crate::domain::ports`]),
//! keeping the application logic decoupled from concrete operating system or FFmpeg implementations.
//!
//! Submodules:
//! - [`dump_iso`]: [`DumpIsoUseCase`] for 1:1 archival disc image dumps.
//! - [`probe_disc`]: [`ProbeDiscUseCase`] for querying disc metadata and analyzing title tracks.
//! - [`rip_single_title`]: [`RipSingleTitleUseCase`] for single movie ripping pipelines.
//! - [`rip_tv_batch`]: [`RipTvBatchUseCase`] for batch ripping multi-episode television series.
//! - [`drive_pool`]: [`DrivePool`] for managing concurrent multi-drive ripping operations.

pub mod cli_runner;
pub mod drive_pool;
pub mod dump_iso;
pub mod probe_disc;
pub mod rip_single_title;
pub mod rip_tv_batch;

pub use cli_runner::{CliApplicationRunner, CliMetadataResolver};
pub use drive_pool::DrivePool;
pub use dump_iso::{DumpIsoRequest, DumpIsoRequestBuilder, DumpIsoUseCase};
pub use probe_disc::{DiscProbeResult, ProbeDiscUseCase};
pub use rip_single_title::{
    RipSingleTitleRequest, RipSingleTitleRequestBuilder, RipSingleTitleUseCase,
};
pub use rip_tv_batch::{RipTvBatchRequest, RipTvBatchRequestBuilder, RipTvBatchUseCase};

#[cfg(test)]
mod tests;
