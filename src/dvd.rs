/**
 * @file dvd.rs
 * @brief DVD drive resolution and Windows API volume label detection.
 */

use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

/// Resolves and normalizes the DVD drive input path (e.g., adding trailing backslash if letter given).
pub fn normalize_dvd_path(input: &str) -> PathBuf {
    let mut dvd_path = PathBuf::from(input);
    if dvd_path.to_string_lossy().ends_with(':') {
        dvd_path = PathBuf::from(format!("{}\\", input));
    }
    dvd_path
}

/// Retrieves the local volume label of a DVD drive using the Windows API.
pub fn get_volume_label(root_path: &str) -> Option<String> {
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
