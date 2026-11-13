//! Concrete optical drive acoustic supervisor implementing SCSI speed control and laser thermal management.

use crate::domain::ports::acoustic_supervisor::{AcousticSupervisor, DriveAcousticProfile};
use anyhow::Result;
use std::path::Path;
use std::time::Duration;

/// Concrete acoustic supervisor communicating with optical drive firmware via OS IOCTL / SCSI calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScsiSpeedController;

impl ScsiSpeedController {
    pub fn new() -> Self {
        Self
    }
}

impl AcousticSupervisor for ScsiSpeedController {
    fn apply_profile(&self, drive_path: &Path, profile: DriveAcousticProfile) -> Result<()> {
        let kbps = profile.target_dvd_read_kbps();

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
            use windows::Win32::Storage::FileSystem::{
                CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            };
            use windows::Win32::System::IO::DeviceIoControl;
            use windows::core::PCWSTR;

            let drive_str = drive_path.to_string_lossy();
            let letter = drive_str
                .chars()
                .find(|c| c.is_ascii_alphabetic())
                .unwrap_or('D')
                .to_ascii_uppercase();

            let device_path: Vec<u16> = format!(r"\\.\{}:", letter)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                // Try opening with Read/Write permissions, falling back to Read-only
                let handle = CreateFileW(
                    PCWSTR::from_raw(device_path.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    Default::default(),
                    HANDLE::default(),
                );

                let handle = if handle.is_err() {
                    CreateFileW(
                        PCWSTR::from_raw(device_path.as_ptr()),
                        GENERIC_READ.0,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        None,
                        OPEN_EXISTING,
                        Default::default(),
                        HANDLE::default(),
                    )
                } else {
                    handle
                };

                if let Ok(h) = handle {
                    // IOCTL_CDROM_SET_SPEED = 0x0002403E
                    const IOCTL_CDROM_SET_SPEED: u32 = 0x0002403E;

                    #[repr(C)]
                    struct CdromSetSpeed {
                        request_type: u32,
                        read_speed: u16,
                        write_speed: u16,
                        rotation_control: u32,
                    }

                    let speed_cmd = CdromSetSpeed {
                        request_type: 0, // CdromSetSpeed
                        read_speed: (kbps.min(0xFFFF)) as u16,
                        write_speed: 0xFFFF, // Default write speed
                        rotation_control: 0, // CdromDefaultRotation
                    };

                    let mut bytes_returned = 0u32;
                    let success = DeviceIoControl(
                        h,
                        IOCTL_CDROM_SET_SPEED,
                        Some(&speed_cmd as *const _ as *const _),
                        std::mem::size_of::<CdromSetSpeed>() as u32,
                        None,
                        0,
                        Some(&mut bytes_returned),
                        None,
                    );

                    let _ = CloseHandle(h);

                    if success.is_ok() {
                        println!(
                            "[Optical Supervisor] Applied acoustic profile '{}' (Target: {} KB/s) to drive {}:",
                            profile.display_label(),
                            kbps,
                            letter
                        );
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let dev_str = drive_path.to_string_lossy();
            let target_x = (kbps / 1385).max(1) as i32;

            if let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&*dev_str)
            {
                use std::os::unix::io::AsRawFd;
                // CDROM_SELECT_SPEED = 0x5322
                const CDROM_SELECT_SPEED: u64 = 0x5322;
                unsafe {
                    let res = libc::ioctl(file.as_raw_fd(), CDROM_SELECT_SPEED as _, target_x);
                    if res == 0 {
                        println!(
                            "[Optical Supervisor] Applied acoustic profile '{}' ({}x speed) to {}",
                            profile.display_label(),
                            target_x,
                            dev_str
                        );
                    }
                }
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            let _ = (drive_path, kbps);
        }

        Ok(())
    }

    fn handle_read_error_thermal_rest(&self, consecutive_errors: u32) -> u64 {
        if consecutive_errors >= 20 {
            // Apply progressive laser thermal rest pause
            let multiplier = (consecutive_errors / 20).min(5);
            let pause_ms = (multiplier * 200) as u64;
            std::thread::sleep(Duration::from_millis(pause_ms));
            println!(
                "[Optical Thermal Guard] Consecutive read errors ({}); applying {}ms laser cool-down pause.",
                consecutive_errors, pause_ms
            );
            pause_ms
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scsi_controller_thermal_rest_calculation() {
        let supervisor = ScsiSpeedController::new();

        // Under threshold: 0ms
        assert_eq!(supervisor.handle_read_error_thermal_rest(0), 0);
        assert_eq!(supervisor.handle_read_error_thermal_rest(19), 0);

        // At or above threshold: progressive pause
        assert!(supervisor.handle_read_error_thermal_rest(20) >= 200);
        assert!(supervisor.handle_read_error_thermal_rest(40) >= 400);
    }

    #[test]
    fn test_scsi_controller_apply_profile_dummy_path() {
        let supervisor = ScsiSpeedController::new();
        // Should safely execute and not panic even on nonexistent path
        let res = supervisor.apply_profile(Path::new("Z:\\NonExistent"), DriveAcousticProfile::Silent);
        assert!(res.is_ok());
    }
}

