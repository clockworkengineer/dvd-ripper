use std::process::Command;

#[test]
fn test_cli_help_flag() {
    let bin = env!("CARGO_BIN_EXE_dvd-ripper");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("Failed to execute dvd-ripper binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--cli"));
    assert!(stdout.contains("--daemon"));
    assert!(stdout.contains("--transcode"));
    assert!(stdout.contains("--mkv"));
    assert!(stdout.contains("--iso"));
    assert!(stdout.contains("--tv"));
}

#[test]
fn test_cli_version_flag() {
    let bin = env!("CARGO_BIN_EXE_dvd-ripper");
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .expect("Failed to execute dvd-ripper binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dvd-ripper"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_nonexistent_drive_fails_gracefully() {
    let bin = env!("CARGO_BIN_EXE_dvd-ripper");
    let output = Command::new(bin)
        .args(["--cli", "Z:\\DefinitelyNonExistentDrive\\NoDisc"])
        .output()
        .expect("Failed to execute dvd-ripper binary");

    // Must not panic, must exit with error status code
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stdout, stderr);
    assert!(combined.contains("does not exist") || combined.contains("Error"));
}

#[test]
fn test_installer_help_flag() {
    let bin = env!("CARGO_BIN_EXE_dvd-ripper-installer");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("Failed to execute dvd-ripper-installer binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cross-Platform Installer for DVD Ripper"));
    assert!(stdout.contains("--user"));
    assert!(stdout.contains("--system"));
    assert!(stdout.contains("--service"));
    assert!(stdout.contains("--uninstall"));
}

#[test]
fn test_installer_version_flag() {
    let bin = env!("CARGO_BIN_EXE_dvd-ripper-installer");
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .expect("Failed to execute dvd-ripper-installer binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dvd-ripper-installer"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}
