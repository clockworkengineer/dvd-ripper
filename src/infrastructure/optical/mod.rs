pub mod platform;
pub mod windows;

pub use windows::{
    PhysicalOpticalDrive, SystemDiscInspector, SystemDriveDetector, SystemTrayController,
};
