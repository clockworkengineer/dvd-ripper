//! Hardware-accelerated and concurrent streaming pipeline infrastructure.

pub mod stream_tee;

pub use stream_tee::{OPTICAL_TEE_CHUNK_SIZE, tee_stream};

