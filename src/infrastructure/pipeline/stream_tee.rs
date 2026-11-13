//! High-performance real-time stream splitter (Tee) for simultaneous dual-output (DRY/SRP).
//!
//! Enables reading optical disc sectors exactly ONCE, concurrently branching the byte stream
//! into a lossless 1:1 archival master sink and a live hardware transcoder pipe without disk re-reads.

use crate::domain::entities::error::DvdError;
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Sector chunk size for optical reading (64 sectors * 2048 bytes = 128 KB).
pub const OPTICAL_TEE_CHUNK_SIZE: usize = 64 * 2048;

/// Replicates an input readable stream across two concurrent output writable sinks.
///
/// Returns total bytes written to `(sink1, sink2)` upon successful completion.
pub fn tee_stream<R, W1, W2>(
    mut reader: R,
    mut sink1: W1,
    mut sink2: W2,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<(u64, u64)>
where
    R: Read,
    W1: Write,
    W2: Write,
{
    let mut buffer = vec![0u8; OPTICAL_TEE_CHUNK_SIZE];
    let mut total_sink1: u64 = 0;
    let mut total_sink2: u64 = 0;

    loop {
        if let Some(flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            return Err(DvdError::Cancelled.into());
        }

        let bytes_read = match reader.read(&mut buffer) {
            Ok(0) => break, // EOF reached
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e).context("Error reading source optical stream in StreamTee"),
        };

        let chunk = &buffer[..bytes_read];

        sink1
            .write_all(chunk)
            .context("StreamTee: failed to write chunk to Archival Master sink")?;
        total_sink1 += bytes_read as u64;

        sink2
            .write_all(chunk)
            .context("StreamTee: failed to write chunk to Transcoder Input sink")?;
        total_sink2 += bytes_read as u64;
    }

    sink1.flush().context("StreamTee: failed to flush Archival Master sink")?;
    sink2.flush().context("StreamTee: failed to flush Transcoder Input sink")?;

    Ok((total_sink1, total_sink2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_tee_stream_roundtrip() {
        let input_data = b"Hello, World! This is a test of the single-pass dual-output stream tee.";
        let reader = Cursor::new(input_data);
        let mut sink1 = Vec::new();
        let mut sink2 = Vec::new();

        let (b1, b2) = tee_stream(reader, &mut sink1, &mut sink2, None).unwrap();

        assert_eq!(b1 as usize, input_data.len());
        assert_eq!(b2 as usize, input_data.len());
        assert_eq!(&sink1[..], input_data);
        assert_eq!(&sink2[..], input_data);
    }

    #[test]
    fn test_tee_stream_cancellation() {
        let input_data = vec![0xAAu8; OPTICAL_TEE_CHUNK_SIZE * 4];
        let reader = Cursor::new(input_data);
        let mut sink1 = Vec::new();
        let mut sink2 = Vec::new();
        let cancel = Arc::new(AtomicBool::new(true));

        let res = tee_stream(reader, &mut sink1, &mut sink2, Some(&cancel));
        assert!(res.is_err());
    }
}

