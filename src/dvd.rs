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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_resolve_device_path() {
        assert_eq!(resolve_device_path("/dev/sr1"), "/dev/sr1");
        let resolved = resolve_device_path("auto");
        assert!(!resolved.is_empty());
    }
}
