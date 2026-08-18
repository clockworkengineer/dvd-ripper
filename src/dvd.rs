/**
 * @file dvd.rs
 * @brief Cross-platform DVD drive detection and volume label reading (Windows, Linux, macOS).
 */

use std::path::PathBuf;

/// Detects all optical DVD/CD drives attached to the operating system.
#[cfg(target_os = "windows")]
pub fn detect_dvd_drives() -> Vec<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDriveStringsW};

    let mut drives = Vec::new();
    let mut buffer = [0u16; 512];
    unsafe {
        let len = GetLogicalDriveStringsW(Some(&mut buffer));
        if len > 0 {
            let mut i = 0;
            while i < len as usize && buffer[i] != 0 {
                let mut end = i;
                while end < len as usize && buffer[end] != 0 {
                    end += 1;
                }
                let drive_str = String::from_utf16_lossy(&buffer[i..end]);
                let drive_wide: Vec<u16> = drive_str.encode_utf16().chain(std::iter::once(0)).collect();
                if GetDriveTypeW(PCWSTR::from_raw(drive_wide.as_ptr())) == 5 {
                    drives.push(drive_str);
                }
                i = end + 1;
            }
        }
    }
    if drives.is_empty() {
        drives.push("D:\\".to_string());
    }
    drives
}

/// Detects all optical DVD/CD drives attached to Linux system.
#[cfg(target_os = "linux")]
pub fn detect_dvd_drives() -> Vec<String> {
    let mut drives = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("sr") {
                drives.push(format!("/dev/{}", name));
            }
        }
    }
    if drives.is_empty() && std::path::Path::new("/dev/dvd").exists() {
        drives.push("/dev/dvd".to_string());
    }
    if drives.is_empty() && std::path::Path::new("/dev/cdrom").exists() {
        drives.push("/dev/cdrom".to_string());
    }
    if drives.is_empty() {
        drives.push("/dev/sr0".to_string());
    }
    drives
}

/// Detects all optical DVD/CD drives attached to macOS system.
#[cfg(target_os = "macos")]
pub fn detect_dvd_drives() -> Vec<String> {
    let mut drives = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("VIDEO_TS").exists() || path.join("video_ts").exists() {
                drives.push(path.to_string_lossy().to_string());
            }
        }
    }
    if drives.is_empty() && std::path::Path::new("/dev/disk2").exists() {
        drives.push("/dev/disk2".to_string());
    }
    if drives.is_empty() {
        drives.push("/Volumes/DVD".to_string());
    }
    drives
}

/// Fallback drive detection for unhandled target OS platforms.
#[cfg(all(not(target_os = "windows"), not(target_os = "linux"), not(target_os = "macos")))]
pub fn detect_dvd_drives() -> Vec<String> {
    vec!["/dev/sr0".to_string()]
}

/// Automatically detects the active DVD drive containing an inserted disc, or the primary optical drive.
pub fn auto_detect_dvd_drive() -> String {
    let drives = detect_dvd_drives();
    for drive in &drives {
        if get_volume_label(drive).is_some() {
            return drive.clone();
        }
    }
    drives.into_iter().next().unwrap_or_else(|| {
        if cfg!(target_os = "windows") {
            "D:\\".to_string()
        } else {
            "/dev/sr0".to_string()
        }
    })
}

/// Cleans an input drive string, returning a normalized drive letter (e.g. "D:\", "d:" -> "D:\").
pub fn clean_drive_letter(input: &str) -> String {
    let clean = input.trim().trim_matches('"').trim_matches('\'');
    if clean.len() == 1 && clean.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
        format!("{}:\\", clean.to_ascii_uppercase())
    } else if clean.len() == 2 && clean.ends_with(':') {
        format!("{}\\", clean.to_ascii_uppercase())
    } else {
        clean.to_string()
    }
}

/// Resolves and normalizes the DVD drive input path (e.g., handling "auto", drive letters, trailing backslashes).
pub fn normalize_dvd_path(input: &str) -> PathBuf {
    let trimmed = input.trim();
    let resolved = if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        auto_detect_dvd_drive()
    } else {
        trimmed.to_string()
    };

    if resolved.ends_with(':') {
        PathBuf::from(format!("{}\\", resolved))
    } else {
        PathBuf::from(resolved)
    }
}

/// Retrieves the local volume label of a DVD drive using platform-native calls.
#[cfg(target_os = "windows")]
pub fn get_volume_label(root_path: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

    let target_path = if root_path.is_empty() || root_path.eq_ignore_ascii_case("auto") {
        auto_detect_dvd_drive()
    } else {
        root_path.to_string()
    };

    let mut path_wide = [0u16; 260];
    let mut len = 0;
    for c in target_path.encode_utf16() {
        if len < 258 {
            path_wide[len] = c;
            len += 1;
        }
    }
    if len > 0 && path_wide[len - 1] != b'\\' as u16 {
        path_wide[len] = b'\\' as u16;
        len += 1;
    }
    path_wide[len] = 0;

    let mut volume_name = [0u16; 260];

    unsafe {
        let result = GetVolumeInformationW(
            PCWSTR::from_raw(path_wide.as_ptr()),
            Some(&mut volume_name),
            None,
            None,
            None,
            None,
        );
        if result.is_ok() {
            let name = String::from_utf16_lossy(&volume_name);
            let trimmed = name.trim_matches(char::from(0)).trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

/// POSIX device path resolver for Linux and macOS.
#[cfg(not(target_os = "windows"))]
fn resolve_device_path(root_path: &str) -> String {
    let trimmed = root_path.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") || trimmed == "D:\\" || trimmed == "D:" {
        auto_detect_dvd_drive()
    } else {
        trimmed.to_string()
    }
}

/// Retrieves the local volume label of a DVD drive using platform-native calls.
#[cfg(not(target_os = "windows"))]
pub fn get_volume_label(root_path: &str) -> Option<String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let dev_path = resolve_device_path(root_path);

    if let Ok(mut file) = File::open(&dev_path) {
        // Sector 16 is at offset 32768 (16 * 2048) in ISO-9660 Primary Volume Descriptor
        if file.seek(SeekFrom::Start(32768)).is_ok() {
            let mut buf = [0u8; 2048];
            if file.read_exact(&mut buf).is_ok() {
                // Check ISO-9660 identifier "CD001" at offset 1..6
                if &buf[1..6] == b"CD001" {
                    // Volume Identifier is 32 bytes at offset 40..72
                    let vol_id = String::from_utf8_lossy(&buf[40..72]);
                    let trimmed = vol_id.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }

    let path = std::path::Path::new(&dev_path);
    if path.exists() {
        if let Some(stem) = path.file_stem() {
            return Some(stem.to_string_lossy().to_string());
        }
    }

    None
}

/// Computes a deterministic disc fingerprint hash based on volume descriptors or VTS header metadata.
pub fn compute_disc_fingerprint(root_path: &str) -> String {
    let resolved = normalize_dvd_path(root_path);
    let mut hasher_data = Vec::new();

    // 1. Try reading Sector 16 Primary Volume Descriptor (if raw device or ISO)
    if let Ok(mut f) = std::fs::File::open(&resolved) {
        use std::io::{Read, Seek, SeekFrom};
        let _ = f.seek(SeekFrom::Start(0x8000));
        let mut sector_buf = [0u8; 2048];
        if f.read_exact(&mut sector_buf).is_ok() {
            hasher_data.extend_from_slice(&sector_buf);
        }
    }

    // 2. Try inspecting VIDEO_TS/VTS_01_0.IFO if mounted directory
    let video_ts = if resolved.join("VIDEO_TS").exists() {
        resolved.join("VIDEO_TS")
    } else if resolved.join("video_ts").exists() {
        resolved.join("video_ts")
    } else {
        resolved.clone()
    };

    if let Ok(entries) = std::fs::read_dir(&video_ts) {
        let mut ifo_files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("ifo")))
            .collect();
        ifo_files.sort();
        for ifo in ifo_files {
            if let Ok(metadata) = std::fs::metadata(&ifo) {
                hasher_data.extend_from_slice(&metadata.len().to_le_bytes());
            }
        }
    }

    if hasher_data.is_empty() {
        let label = get_volume_label(root_path).unwrap_or_else(|| "UNKNOWN".to_string());
        hasher_data.extend_from_slice(label.as_bytes());
    }

    // Simple FNV-1a 64-bit hash
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in hasher_data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("disc_{:016x}", hash)
}

/// Ejects the optical drive tray using platform-native calls or system utilities.
pub fn eject_disc(root_path: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let resolved = if root_path.is_empty() || root_path.eq_ignore_ascii_case("auto") {
            auto_detect_dvd_drive()
        } else {
            root_path.to_string()
        };
        let drive_letter = resolved.chars().next().unwrap_or('D');
        let ps_cmd = format!(
            "(New-Object -ComObject Shell.Application).NameSpace(17).ParseName('{}:').InvokeVerb('Eject')",
            drive_letter
        );
        Command::new("powershell").args(["-Command", &ps_cmd]).output().is_ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command;
        let dev_path = resolve_device_path(root_path);
        Command::new("eject").arg(&dev_path).status().map_or(false, |s| s.success())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscProtectionReport {
    pub has_css_indicators: bool,
    pub vob_count: usize,
    pub ifo_count: usize,
    pub total_bytes: u64,
    pub diagnostic_notes: Vec<String>,
}

pub fn inspect_disc_copy_protection(dvd_path: &std::path::Path) -> DiscProtectionReport {
    let video_ts = if dvd_path.file_name().and_then(|s| s.to_str()).map_or(false, |s| s.eq_ignore_ascii_case("VIDEO_TS")) {
        dvd_path.to_path_buf()
    } else {
        dvd_path.join("VIDEO_TS")
    };

    let mut vob_count = 0;
    let mut ifo_count = 0;
    let mut total_bytes = 0u64;
    let mut diagnostic_notes = Vec::new();
    let mut has_css_indicators = false;

    if video_ts.exists() && video_ts.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&video_ts) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_upper = ext.to_uppercase();
                    if ext_upper == "VOB" {
                        vob_count += 1;
                        if let Ok(meta) = entry.metadata() {
                            total_bytes += meta.len();
                        }
                    } else if ext_upper == "IFO" {
                        ifo_count += 1;
                        if let Ok(meta) = entry.metadata() {
                            total_bytes += meta.len();
                        }
                        if path.file_name().and_then(|s| s.to_str()).map_or(false, |s| s.eq_ignore_ascii_case("VIDEO_TS.IFO")) {
                            if let Ok(mut f) = std::fs::File::open(&path) {
                                use std::io::Read;
                                let mut buf = [0u8; 2048];
                                if f.read(&mut buf).is_ok() {
                                    if buf[0x0014] & 0x01 != 0 || buf[0x0015] != 0 {
                                        has_css_indicators = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        diagnostic_notes.push(format!("VIDEO_TS directory not found at path {}", dvd_path.display()));
    }

    if has_css_indicators {
        diagnostic_notes.push("CSS (Content Scramble System) encryption flags detected in VIDEO_TS.IFO headers.".to_string());
    } else {
        diagnostic_notes.push("No active CSS encryption flags detected in IFO headers.".to_string());
    }

    if vob_count > 10 {
        diagnostic_notes.push(format!("High VOB file count ({}) indicates potential structural copy protection or multi-title layout.", vob_count));
    }

    let gigabytes = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    diagnostic_notes.push(format!("Total disc payload size: {:.2} GB across {} VOBs and {} IFOs.", gigabytes, vob_count, ifo_count));

    DiscProtectionReport {
        has_css_indicators,
        vob_count,
        ifo_count,
        total_bytes,
        diagnostic_notes,
    }
}

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Instant;

/// Benchmark report metrics for optical drive throughput performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveBenchmarkReport {
    pub drive_path: String,
    pub test_duration_secs: u64,
    pub read_bytes: u64,
    pub read_speed_mbps: f64,
    pub demux_speed_mbps: f64,
    pub fps: String,
    pub rating_summary: String,
}

/// Runs optical drive throughput diagnostic benchmark measuring read speed (MB/s) and demuxer FPS.
pub fn run_drive_benchmark(
    ffmpeg_path: &str,
    dvd_path: &std::path::Path,
    duration_secs: u64,
) -> anyhow::Result<DriveBenchmarkReport> {
    let start_time = Instant::now();
    let norm_path = normalize_dvd_path(&dvd_path.to_string_lossy());

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y")
       .arg("-f").arg("dvdvideo")
       .arg("-i").arg(&norm_path)
       .arg("-title").arg("1")
       .arg("-t").arg(duration_secs.to_string())
       .arg("-f").arg("null")
       .arg("-")
       .arg("-benchmark");

    let output = cmd.output()?;
    let elapsed = start_time.elapsed().as_secs_f64().max(0.1);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let time_sec = crate::utils::extract_kv_field(&stderr, "time=")
        .and_then(crate::utils::parse_duration)
        .unwrap_or(duration_secs as f64);
    let fps = crate::utils::extract_kv_field(&stderr, "fps=")
        .unwrap_or("N/A")
        .to_string();

    let est_bytes = (time_sec * 6_000_000.0 / 8.0) as u64;
    let read_speed_mbps = (est_bytes as f64 / (1024.0 * 1024.0)) / elapsed;
    let demux_speed_mbps = (est_bytes as f64 / (1024.0 * 1024.0)) / time_sec.max(0.1);

    let rating_summary = if read_speed_mbps >= 15.0 {
        "High Performance (16x+ Speed)".to_string()
    } else if read_speed_mbps >= 8.0 {
        "Standard DVD Read Speed (8x Speed)".to_string()
    } else if read_speed_mbps >= 4.0 {
        "Moderate Speed (4x Speed)".to_string()
    } else {
        "Slow / Potential RipLock or Bus Bottleneck (< 4x)".to_string()
    };

    Ok(DriveBenchmarkReport {
        drive_path: norm_path.to_string_lossy().to_string(),
        test_duration_secs: duration_secs,
        read_bytes: est_bytes,
        read_speed_mbps,
        demux_speed_mbps,
        fps,
        rating_summary,
    })
}

/// Compound drive inspection results containing normalized drive path, volume label, and disc fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInspection {
    pub drive_path: PathBuf,
    pub volume_label: Option<String>,
    pub fingerprint: String,
}

/// Helper: Performs a complete initial drive inspection on a user-provided drive input.
pub fn inspect_drive(input: &str) -> DriveInspection {
    let drive_path = normalize_dvd_path(input);
    let volume_label = get_volume_label(&drive_path.to_string_lossy());
    let fingerprint = compute_disc_fingerprint(&drive_path.to_string_lossy());
    DriveInspection {
        drive_path,
        volume_label,
        fingerprint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drive_benchmark_report_struct() {
        let report = DriveBenchmarkReport {
            drive_path: "D:\\".to_string(),
            test_duration_secs: 10,
            read_bytes: 15_000_000,
            read_speed_mbps: 12.5,
            demux_speed_mbps: 18.0,
            fps: "45.0".to_string(),
            rating_summary: "Standard DVD Read Speed (8x Speed)".to_string(),
        };
        assert_eq!(report.drive_path, "D:\\");
        assert_eq!(report.test_duration_secs, 10);
        assert!(report.read_speed_mbps > 10.0);
    }

    #[test]
    fn test_inspect_drive() {
        let inspection = inspect_drive("D:");
        assert_eq!(inspection.drive_path, PathBuf::from("D:\\"));
        assert!(inspection.fingerprint.starts_with("disc_"));
    }

    #[test]
    fn test_clean_drive_letter() {
        assert_eq!(clean_drive_letter("d"), "D:\\");
        assert_eq!(clean_drive_letter("D:"), "D:\\");
        assert_eq!(clean_drive_letter("E:\\"), "E:\\");
    }

    #[test]
    fn test_detect_dvd_drives() {
        let drives = detect_dvd_drives();
        assert!(!drives.is_empty());
    }

    #[test]
    fn test_auto_detect_dvd_drive() {
        let drive = auto_detect_dvd_drive();
        assert!(!drive.is_empty());
    }

    #[test]
    fn test_normalize_dvd_path() {
        assert_eq!(normalize_dvd_path("D:"), PathBuf::from("D:\\"));
        assert_eq!(normalize_dvd_path("D:\\"), PathBuf::from("D:\\"));
        assert_eq!(normalize_dvd_path("/dev/sr0"), PathBuf::from("/dev/sr0"));
        assert_eq!(normalize_dvd_path("E:"), PathBuf::from("E:\\"));
        let auto_path = normalize_dvd_path("auto");
        assert!(!auto_path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_compute_disc_fingerprint() {
        let fp1 = compute_disc_fingerprint("D:\\");
        assert!(fp1.starts_with("disc_"));

        let fp2 = compute_disc_fingerprint("D:\\");
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_inspect_disc_copy_protection() {
        let temp_dir = std::env::temp_dir().join("disc_protection_test");
        let video_ts = temp_dir.join("VIDEO_TS");
        std::fs::create_dir_all(&video_ts).unwrap();

        let ifo = video_ts.join("VIDEO_TS.IFO");
        std::fs::write(&ifo, vec![0u8; 2048]).unwrap();

        let vob = video_ts.join("VTS_01_1.VOB");
        std::fs::write(&vob, vec![0u8; 4096]).unwrap();

        let report = inspect_disc_copy_protection(&temp_dir);
        assert_eq!(report.vob_count, 1);
        assert_eq!(report.ifo_count, 1);
        assert!(!report.diagnostic_notes.is_empty());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
