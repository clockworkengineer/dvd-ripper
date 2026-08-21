/**
 * @file installer.rs
 * @brief Standalone Portable Multi-OS Rust Installer & Setup Engine for DVD Ripper.
 * Supporting Windows, Linux, and macOS (User & System-wide installation, PATH management, FFmpeg auditing, systemd/udev setup, and uninstallation).
 */

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Context, Result};
use clap::Parser;

/// Standalone Cross-Platform Installer for DVD Ripper.
#[derive(Parser, Debug)]
#[command(
    name = "dvd-ripper-installer",
    version,
    about = "Cross-Platform Installer for DVD Ripper (Windows, Linux, macOS)"
)]
struct InstallerArgs {
    /// Install system-wide (requires Administrator/root privileges)
    #[arg(long)]
    system: bool,

    /// Install for current user (default)
    #[arg(long, default_value_t = true)]
    user: bool,

    /// Custom target installation directory
    #[arg(short, long)]
    dir: Option<PathBuf>,

    /// Uninstall DVD Ripper from system
    #[arg(short = 'u', long)]
    uninstall: bool,

    /// Install Linux systemd service and udev rules (Linux system-wide mode)
    #[arg(long)]
    service: bool,

    /// Non-interactive mode (automatically answer yes to prompts)
    #[arg(short = 'y', long)]
    yes: bool,
}

fn main() -> Result<()> {
    let args = InstallerArgs::parse();

    println!("====================================================");
    println!("   📀 DVD Ripper Portable Multi-OS Installer");
    println!("====================================================\n");

    if args.uninstall {
        return run_uninstall(&args);
    }

    // 1. Audit FFmpeg availability
    audit_ffmpeg()?;

    // 2. Resolve source binary executable
    let source_binary = find_source_binary()?;
    println!("[+] Found source binary: {}", source_binary.display());

    // 3. Resolve installation destination directory
    let target_dir = resolve_target_dir(&args)?;
    let binary_name = target_binary_name();
    let target_binary = target_dir.join(binary_name);

    println!("[+] Installation Target Path: {}\n", target_binary.display());

    if !args.yes {
        print!("Proceed with installation? [Y/n]: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        let trimmed = input.trim().to_lowercase();
        if !trimmed.is_empty() && trimmed != "y" && trimmed != "yes" {
            println!("Installation aborted by user.");
            return Ok(());
        }
    }

    // 4. Copy executable binary
    println!("\n[1/4] Copying binary executable...");
    if let Some(parent) = target_binary.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create destination directory: {}", parent.display()))?;
    }

    // If overwriting an existing running binary on Windows, remove or rename it first
    if target_binary.exists() {
        let _ = fs::remove_file(&target_binary);
    }

    fs::copy(&source_binary, &target_binary)
        .with_context(|| format!("Failed to copy binary from {} to {}", source_binary.display(), target_binary.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target_binary)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_binary, perms)?;
        println!("      [+] Set executable permissions (0755)");
    }

    println!("      [+] Installed {}", target_binary.display());

    // 5. Configure PATH Environment
    println!("\n[2/4] Configuring System PATH...");
    configure_path(&target_dir)?;

    // 6. Linux Systemd / Udev Integration (if requested or running as root)
    println!("\n[3/4] Checking System Appliance Integration...");
    configure_system_services(&args)?;

    // 7. Verification Summary
    println!("\n[4/4] Finalizing Installation...");
    println!("\n🎉 DVD Ripper installation successfully completed!");
    println!("     Executable path : {}", target_binary.display());
    println!("     To run GUI      : {}", binary_name);
    println!("     To run CLI      : {} --cli D:\\", binary_name);
    println!("     To run Daemon   : {} --daemon\n", binary_name);

    Ok(())
}

/// Checks if FFmpeg is installed and accessible in the system PATH.
fn audit_ffmpeg() -> Result<()> {
    print!("[+] Auditing FFmpeg dependency... ");
    io::stdout().flush().ok();

    let output = Command::new("ffmpeg").arg("-version").output();

    match output {
        Ok(out) if out.status.success() => {
            let stderr_or_stdout = String::from_utf8_lossy(&out.stdout);
            let first_line = stderr_or_stdout.lines().next().unwrap_or("FFmpeg detected");
            println!("OK");
            println!("      -> {}", first_line);
        }
        _ => {
            println!("NOT FOUND");
            println!("\n[!] WARNING: FFmpeg was not detected in your system PATH.");
            println!("    dvd-ripper requires FFmpeg built with the 'dvdvideo' demuxer enabled.");
            println!("    Please install FFmpeg using your package manager:");
            if cfg!(windows) {
                println!("    • Windows (WinGet)  : winget install FFmpeg");
                println!("    • Windows (Chocolatey): choco install ffmpeg");
            } else if cfg!(target_os = "macos") {
                println!("    • macOS (Homebrew)  : brew install ffmpeg");
            } else {
                println!("    • Ubuntu/Debian     : sudo apt update && sudo apt install ffmpeg");
                println!("    • Fedora/RHEL       : sudo dnf install ffmpeg");
                println!("    • Arch Linux        : sudo pacman -S ffmpeg");
            }
            println!();
        }
    }
    Ok(())
}

/// Resolves source binary executable location.
fn find_source_binary() -> Result<PathBuf> {
    let binary_name = if cfg!(windows) { "dvd-ripper.exe" } else { "dvd-ripper" };
    let current_exe = env::current_exe().unwrap_or_default();
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let candidates = [
        current_dir.join(binary_name),
        current_dir.join("target").join("release").join(binary_name),
        current_dir.join("target").join("debug").join(binary_name),
        current_exe.parent().map(|p| p.join(binary_name)).unwrap_or_default(),
    ];

    for candidate in &candidates {
        if candidate.exists() && candidate.is_file() {
            if candidate != &current_exe {
                return Ok(candidate.clone());
            }
        }
    }

    Err(anyhow!(
        "Could not locate 'dvd-ripper' executable binary. Please build the project first using 'cargo build --release'."
    ))
}

/// Resolves target installation directory based on OS and installation mode.
fn resolve_target_dir(args: &InstallerArgs) -> Result<PathBuf> {
    if let Some(ref custom_dir) = args.dir {
        return Ok(custom_dir.clone());
    }

    if args.system {
        if cfg!(windows) {
            let program_files = env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
            Ok(PathBuf::from(program_files).join("DVD Ripper"))
        } else {
            Ok(PathBuf::from("/usr/local/bin"))
        }
    } else {
        if cfg!(windows) {
            let local_app_data = env::var("LOCALAPPDATA")
                .or_else(|_| env::var("USERPROFILE").map(|p| format!("{}\\AppData\\Local", p)))
                .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
            Ok(PathBuf::from(local_app_data).join("dvd-ripper").join("bin"))
        } else {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            Ok(PathBuf::from(home).join(".local").join("bin"))
        }
    }
}

/// Adds installation directory to user/system PATH.
fn configure_path(target_dir: &Path) -> Result<()> {
    let target_str = target_dir.to_string_lossy().to_string();

    #[cfg(windows)]
    {
        println!("      [+] Updating Windows User Registry PATH...");
        let safe_target = target_str.replace('\\', "\\\\").replace('\'', "''");
        let ps_cmd = format!(
            "$oldPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
             if (-not $oldPath.Split(';').Contains('{}')) {{ \
                 $newPath = \"$oldPath;{}\"; \
                 [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); \
                 Write-Host 'PATH updated successfully.'; \
             }} else {{ Write-Host 'Directory already present in PATH.'; }}",
            safe_target,
            safe_target
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .output();

        if let Ok(out) = output {
            let msg = String::from_utf8_lossy(&out.stdout);
            println!("      -> {}", msg.trim());
        }
    }

    #[cfg(unix)]
    {
        let path_env = env::var("PATH").unwrap_or_default();
        if path_env.split(':').any(|p| Path::new(p) == target_dir) {
            println!("      [+] Installation directory is already present in PATH.");
        } else {
            println!("      [!] Installation directory '{}' is not yet in PATH.", target_str);
            let home = env::var("HOME").unwrap_or_default();
            let rc_files = [".bashrc", ".zshrc", ".profile"];
            for rc in &rc_files {
                let rc_path = PathBuf::from(&home).join(rc);
                if rc_path.exists() {
                    let export_line = format!("\nexport PATH=\"{}:$PATH\"\n", target_str);
                    if let Ok(content) = fs::read_to_string(&rc_path) {
                        if !content.contains(&target_str) {
                            if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&rc_path) {
                                let _ = f.write_all(export_line.as_bytes());
                                println!("      [+] Added export PATH entry to ~/{}.", rc);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Configures Linux systemd unit & udev rules if installing system-wide.
fn configure_system_services(args: &InstallerArgs) -> Result<()> {
    if cfg!(target_os = "linux") && (args.system || args.service) {
        let service_src = Path::new("contrib/dvd-ripper.service");
        let rules_src = Path::new("contrib/99-dvd-ripper.rules");

        if service_src.exists() {
            let service_dst = Path::new("/etc/systemd/system/dvd-ripper.service");
            if fs::copy(service_src, service_dst).is_ok() {
                println!("      [+] Installed systemd unit: {}", service_dst.display());
                let _ = Command::new("systemctl").arg("daemon-reload").status();
            }
        }

        if rules_src.exists() {
            let rules_dst = Path::new("/etc/udev/rules.d/99-dvd-ripper.rules");
            if fs::copy(rules_src, rules_dst).is_ok() {
                println!("      [+] Installed udev rules: {}", rules_dst.display());
                let _ = Command::new("udevadm").args(["control", "--reload-rules"]).status();
            }
        }
    } else {
        println!("      [+] User installation mode active (systemd/udev setup skipped).");
    }
    Ok(())
}

/// Performs uninstallation of DVD Ripper.
fn run_uninstall(args: &InstallerArgs) -> Result<()> {
    println!("=== DVD Ripper Uninstallation ===");
    let target_dir = resolve_target_dir(args)?;
    let binary_name = target_binary_name();
    let target_binary = target_dir.join(binary_name);

    if target_binary.exists() {
        fs::remove_file(&target_binary)
            .with_context(|| format!("Failed to remove executable: {}", target_binary.display()))?;
        println!("[+] Removed binary: {}", target_binary.display());
    } else {
        println!("[!] Executable not found at: {}", target_binary.display());
    }

    if cfg!(target_os = "linux") && args.system {
        let service_dst = Path::new("/etc/systemd/system/dvd-ripper.service");
        let rules_dst = Path::new("/etc/udev/rules.d/99-dvd-ripper.rules");

        if service_dst.exists() {
            let _ = Command::new("systemctl").args(["stop", "dvd-ripper.service"]).status();
            let _ = Command::new("systemctl").args(["disable", "dvd-ripper.service"]).status();
            let _ = fs::remove_file(service_dst);
            println!("[+] Removed systemd service.");
        }

        if rules_dst.exists() {
            let _ = fs::remove_file(rules_dst);
            println!("[+] Removed udev rules.");
        }

        let _ = Command::new("systemctl").arg("daemon-reload").status();
    }

    if target_dir.exists() {
        if fs::read_dir(&target_dir).map_or(false, |mut i| i.next().is_none()) {
            let _ = fs::remove_dir(&target_dir);
            println!("[+] Removed empty directory: {}", target_dir.display());
        }
    }

    println!("\n✨ DVD Ripper has been successfully uninstalled.");
    Ok(())
}


fn target_binary_name() -> &'static str {

    if cfg!(windows) {
        "dvd-ripper.exe"
    } else {
        "dvd-ripper"
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_resolve_target_dir_user() {
        let args = InstallerArgs {
            system: false,
            user: true,
            dir: None,
            uninstall: false,
            service: false,
            yes: true,
        };
        let dir = resolve_target_dir(&args).unwrap();
        assert!(dir.to_string_lossy().contains("dvd-ripper") || dir.to_string_lossy().contains(".local"));
    }

    #[test]
    fn test_resolve_target_dir_custom() {
        let custom = PathBuf::from("/custom/install/dir");
        let args = InstallerArgs {
            system: false,
            user: true,
            dir: Some(custom.clone()),
            uninstall: false,
            service: false,
            yes: true,
        };
        let dir = resolve_target_dir(&args).unwrap();
        assert_eq!(dir, custom);
    }
}
