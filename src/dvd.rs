/**
 * @file dvd.rs
 * @brief DVD drive resolution and Windows API volume label detection.
 */

use std::path::PathBuf;

/// Resolves and normalizes the DVD drive input path (e.g., adding trailing backslash if letter given).
pub fn normalize_dvd_path(input: &str) -> PathBuf {
    let mut dvd_path = PathBuf::from(input);
    if dvd_path.to_string_lossy().ends_with(':') {
        dvd_path = PathBuf::from(format!("{}\\", input));
    }
    dvd_path
}

/// Retrieves the local volume label of a DVD drive using platform-native calls.
#[cfg(target_os = "windows")]
pub fn get_volume_label(root_path: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

    let mut path_wide: Vec<u16> = root_path.encode_utf16().collect();
    if !path_wide.ends_with(&[b'\\' as u16]) {
        path_wide.push(b'\\' as u16);
    }
    path_wide.push(0);

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

/// Linux POSIX block device ISO-9660 Primary Volume Descriptor reader.
#[cfg(not(target_os = "windows"))]
pub fn get_volume_label(root_path: &str) -> Option<String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let dev_path = if root_path.is_empty() || root_path == "D:\\" || root_path == "D:" {
        "/dev/sr0"
    } else {
        root_path
    };

    if let Ok(mut file) = File::open(dev_path) {
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

    let path = std::path::Path::new(root_path);
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
        let drive_letter = root_path.chars().next().unwrap_or('D');
        let ps_cmd = format!(
            "(New-Object -ComObject Shell.Application).NameSpace(17).ParseName('{}:').InvokeVerb('Eject')",
            drive_letter
        );
        Command::new("powershell").args(["-Command", &ps_cmd]).output().is_ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command;
        let dev_path = if root_path.is_empty() || root_path == "D:\\" || root_path == "D:" {
            "/dev/sr0"
        } else {
            root_path
        };
        Command::new("eject").arg(dev_path).status().map_or(false, |s| s.success())
    }
}
