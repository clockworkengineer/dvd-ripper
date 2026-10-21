use crate::ffmpeg::ProgressEvent;
use anyhow::{Result, anyhow};
/**
 * @file iso.rs
 * @brief Archival 1:1 byte-for-byte optical disc ISO dumping engine.
 */
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

/// Probes Sector 16 (offset 32768) Primary Volume Descriptor to extract exact ISO-9660 payload byte size.
pub fn probe_iso_disc_size<R: Read + Seek>(reader: &mut R) -> Option<u64> {
    if reader.seek(SeekFrom::Start(32768)).is_ok() {
        let mut pvd = [0u8; 2048];
        if reader.read_exact(&mut pvd).is_ok() {
            // Check ISO-9660 Magic Identifier "CD001"
            if &pvd[1..6] == b"CD001" {
                // Logical Block Count at offset 80..84 (Little-Endian)
                let blocks = u32::from_le_bytes([pvd[80], pvd[81], pvd[82], pvd[83]]) as u64;
                // Logical Block Size at offset 128..130 (Little-Endian, typically 2048)
                let block_size = u16::from_le_bytes([pvd[128], pvd[129]]) as u64;
                let actual_block_size = if block_size == 0 { 2048 } else { block_size };
                if blocks > 0 {
                    return Some(blocks * actual_block_size);
                }
            }
        }
    }
    None
}

/// Copies raw optical sectors from a readable stream into an output writer with real-time throughput metrics.
pub fn copy_sectors_with_progress<R: Read, W: Write, F: FnMut(f64, f64, u64)>(
    mut reader: R,
    mut writer: W,
    total_bytes_hint: Option<u64>,
    cancel_flag: Option<&Arc<AtomicBool>>,
    mut progress_cb: F,
) -> Result<u64> {
    const BUFFER_SIZE: usize = 256 * 1024; // 256 KB buffer for high sequential throughput
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total_copied = 0u64;
    let start_time = Instant::now();
    let mut last_progress_report = Instant::now();

    loop {
        if let Some(flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            return Err(anyhow!("ISO dump process cancelled by user"));
        }

        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        writer.write_all(&buffer[..bytes_read])?;
        total_copied += bytes_read as u64;

        if last_progress_report.elapsed().as_millis() >= 250 {
            let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
            let mbps = (total_copied as f64 / (1024.0 * 1024.0)) / elapsed_secs;
            let percent = if let Some(hint) = total_bytes_hint {
                if hint > 0 {
                    ((total_copied as f64 / hint as f64) * 100.0).min(99.9)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            progress_cb(percent, mbps, total_copied);
            last_progress_report = Instant::now();
        }

        // If we know the exact total size from PVD, stop once we've reached it to avoid reading trailing unformatted sectors
        if let Some(hint) = total_bytes_hint
            && total_copied >= hint
        {
            break;
        }
    }

    writer.flush()?;
    Ok(total_copied)
}

/// Resolves the raw device path or filesystem directory for the optical drive.
pub fn resolve_raw_optical_source(input: &str) -> (PathBuf, bool) {
    let trimmed = input.trim();

    #[cfg(target_os = "windows")]
    {
        let drive_letter = trimmed
            .chars()
            .find(|c| c.is_ascii_alphabetic())
            .unwrap_or('D')
            .to_ascii_uppercase();

        let device_path = PathBuf::from(format!(r"\\.\{}:", drive_letter));
        let root_dir = PathBuf::from(format!(r"{}:\", drive_letter));

        // If root directory exists with VIDEO_TS and device path fails, fallback to directory
        (device_path, root_dir.exists())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let path = PathBuf::from(trimmed);
        let is_dir = path.is_dir();
        (path, is_dir)
    }
}

/// Attempts to dump a directory tree containing VIDEO_TS into an ISO using platform-native utilities.
pub fn dump_directory_to_iso(
    dir_path: &Path,
    output_iso_path: &Path,
    volume_label: Option<&str>,
) -> Result<u64> {
    let label = volume_label.unwrap_or("DVD_DISC");

    #[cfg(target_os = "windows")]
    {
        // Use PowerShell with native IMAPI2FS COM object (included in all modern Windows versions)
        let ps_script = format!(
            "$fsi = New-Object -ComObject IMAPI2FS.MsftFileSystemImage; \
             $fsi.FileSystemsToCreate = 3; \
             $fsi.VolumeName = '{}'; \
             $fsi.Root.AddTree('{}', $false); \
             $iso = $fsi.CreateResultImage(); \
             $stream = $iso.ImageStream; \
             $out = [System.IO.File]::OpenWrite('{}'); \
             $buf = New-Object byte[] 65536; \
             while (($read = $stream.Read($buf, 0, $buf.Length)) -gt 0) {{ $out.Write($buf, 0, $read) }}; \
             $out.Close()",
            label.replace('\'', "''"),
            dir_path.display().to_string().replace('\'', "''"),
            output_iso_path.display().to_string().replace('\'', "''")
        );

        let mut cmd = std::process::Command::new("powershell");
        crate::utils::configure_silent_command(&mut cmd);
        let output = cmd.args(["-NoProfile", "-Command", &ps_script]).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("IMAPI2 ISO creation failed: {}", stderr));
        }

        let len = std::fs::metadata(output_iso_path)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(len)
    }

    #[cfg(target_os = "macos")]
    {
        // Use macOS hdiutil
        let mut cmd = std::process::Command::new("hdiutil");
        let status = cmd
            .args([
                "makehybrid",
                "-iso",
                "-joliet",
                "-default-volume-name",
                label,
                "-o",
            ])
            .arg(output_iso_path)
            .arg(dir_path)
            .status()?;
        if !status.success() {
            return Err(anyhow!(
                "hdiutil ISO creation failed with status {:?}",
                status
            ));
        }
        let len = std::fs::metadata(output_iso_path)
            .map(|m| m.len())
            .unwrap_or(0);
        return Ok(len);
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        // Use genisoimage / mkisofs / xorriso on Linux
        let program = if crate::utils::command_exists("genisoimage") {
            "genisoimage"
        } else if crate::utils::command_exists("mkisofs") {
            "mkisofs"
        } else if crate::utils::command_exists("xorriso") {
            "xorriso"
        } else {
            return Err(anyhow!(
                "No ISO generation utility found (install genisoimage, mkisofs, or xorriso)."
            ));
        };

        let mut cmd = std::process::Command::new(program);
        if program == "xorriso" {
            cmd.args(["-as", "mkisofs", "-dvd-video", "-V", label, "-o"])
                .arg(output_iso_path)
                .arg(dir_path);
        } else {
            cmd.args(["-dvd-video", "-V", label, "-o"])
                .arg(output_iso_path)
                .arg(dir_path);
        }

        let status = cmd.status()?;
        if !status.success() {
            return Err(anyhow!(
                "{} ISO creation failed with exit code {:?}",
                program,
                status
            ));
        }
        let len = std::fs::metadata(output_iso_path)
            .map(|m| m.len())
            .unwrap_or(0);
        return Ok(len);
    }
}

/// Dumps a 1:1 bit-for-bit ISO image from the target optical drive to the destination path.
pub fn dump_disc_to_iso(
    input_drive: &str,
    output_iso_path: &Path,
    volume_label: Option<&str>,
    cancel_flag: Option<&Arc<AtomicBool>>,
    progress_tx: Option<&Sender<ProgressEvent>>,
) -> Result<u64> {
    if let Some(parent) = output_iso_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (raw_dev, is_dir_fallback) = resolve_raw_optical_source(input_drive);

    if let Some(tx) = progress_tx {
        let _ = tx.send(ProgressEvent::Log(format!(
            "Starting 1:1 Archival ISO Dump from '{}' to '{}'...",
            input_drive,
            output_iso_path.display()
        )));
    }

    // 1. Attempt raw block sector reading from optical device
    let open_result = {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
                .open(&raw_dev)
        }
        #[cfg(not(target_os = "windows"))]
        {
            File::open(&raw_dev)
        }
    };

    match open_result {
        Ok(mut dev_file) => {
            // Probe Sector 16 for exact size
            let total_size_hint = probe_iso_disc_size(&mut dev_file);
            let _ = dev_file.seek(SeekFrom::Start(0));

            let reader = BufReader::with_capacity(1024 * 1024, dev_file);
            let out_file = File::create(output_iso_path)?;
            let writer = BufWriter::with_capacity(1024 * 1024, out_file);

            let copied = copy_sectors_with_progress(
                reader,
                writer,
                total_size_hint,
                cancel_flag,
                |pct, mbps, bytes| {
                    if let Some(tx) = progress_tx {
                        let _ = tx.send(ProgressEvent::Progress {
                            percent: pct,
                            fps: format!("{:.1} MB/s", mbps),
                            speed: format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                        });
                    }
                },
            )?;

            if let Some(tx) = progress_tx {
                let _ = tx.send(ProgressEvent::Success(output_iso_path.to_path_buf()));
            }

            Ok(copied)
        }
        Err(raw_err) => {
            // 2. If raw device open fails (e.g. mounted directory or restricted permissions), try directory packaging fallback
            let fallback_dir = crate::dvd::normalize_dvd_path(input_drive);
            if is_dir_fallback || fallback_dir.is_dir() {
                if let Some(tx) = progress_tx {
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "Direct raw sector access unavailable ({}); using filesystem image builder fallback...",
                        raw_err
                    )));
                }

                let total = dump_directory_to_iso(&fallback_dir, output_iso_path, volume_label)?;

                if let Some(tx) = progress_tx {
                    let _ = tx.send(ProgressEvent::Success(output_iso_path.to_path_buf()));
                }

                Ok(total)
            } else {
                Err(anyhow!(
                    "Failed to open optical drive '{}' for sector reading: {}",
                    raw_dev.display(),
                    raw_err
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_probe_iso_disc_size_pvd() {
        // Construct a mock ISO stream with 32768 zero bytes followed by Sector 16 PVD
        let mut mock_data = vec![0u8; 32768 + 2048];
        // At offset 32768:
        // Byte 0: Type = 1 (Primary Volume Descriptor)
        mock_data[32768] = 1;
        // Bytes 1..6: "CD001"
        mock_data[32769..32774].copy_from_slice(b"CD001");
        // Offset 80..84: 2,000,000 blocks (approx 4.096 GB)
        let block_count = 2_000_000u32;
        mock_data[32768 + 80..32768 + 84].copy_from_slice(&block_count.to_le_bytes());
        // Offset 128..130: 2048 block size
        let block_size = 2048u16;
        mock_data[32768 + 128..32768 + 130].copy_from_slice(&block_size.to_le_bytes());

        let mut cursor = Cursor::new(mock_data);
        let size = probe_iso_disc_size(&mut cursor);

        assert_eq!(size, Some(2_000_000 * 2048));
    }

    #[test]
    fn test_copy_sectors_with_cancellation() {
        let input_data = vec![0xAAu8; 1024 * 512]; // 512 KB
        let cursor_in = Cursor::new(input_data);
        let mut cursor_out = Cursor::new(Vec::new());

        let cancel_flag = Arc::new(AtomicBool::new(true)); // Pre-cancelled
        let result = copy_sectors_with_progress(
            cursor_in,
            &mut cursor_out,
            Some(1024 * 512),
            Some(&cancel_flag),
            |_, _, _| {},
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[test]
    fn test_copy_sectors_full_roundtrip() {
        let input_data = vec![0x55u8; 1024 * 256];
        let cursor_in = Cursor::new(input_data.clone());
        let mut cursor_out = Cursor::new(Vec::new());

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut progress_called = false;
        let result = copy_sectors_with_progress(
            cursor_in,
            &mut cursor_out,
            Some(input_data.len() as u64),
            Some(&cancel_flag),
            |_, _, _| {
                progress_called = true;
            },
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), input_data.len() as u64);
        assert_eq!(cursor_out.into_inner(), input_data);
    }
}
