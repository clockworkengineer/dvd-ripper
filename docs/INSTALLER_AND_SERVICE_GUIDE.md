# Cross-Platform Installer & Service Guide

`dvd-ripper` includes a standalone, cross-platform installer executable (`dvd-ripper-installer`) built directly from [`src/bin/installer.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/bin/installer.rs).

The installer automates binary installation, system PATH management, FFmpeg dependency auditing, Linux `systemd` background service setup, and Linux `udev` optical disc auto-detection rules across **Windows**, **Linux**, and **macOS**.

---

## 1. Overview & Dual Installation Modes

The installer supports two distinct installation modes:

1. **User Mode (`--user`, Default)**:
   - Installs binaries into user-local directory without requiring Administrator or root privileges.
   - **Windows**: `%LOCALAPPDATA%\dvd-ripper\bin`
   - **Linux / macOS**: `~/.local/bin`
   - Automatically appends installation path to user environment `PATH`.

2. **System-Wide Mode (`--system`)**:
   - Installs binaries system-wide (requires Administrator / root privileges).
   - **Windows**: `%ProgramFiles%\dvd-ripper`
   - **Linux / macOS**: `/usr/local/bin`
   - Configures system-wide `PATH` environment variables.

---

## 2. Installer Command-Line Syntax

```bash
dvd-ripper-installer [OPTIONS]
```

### Options Reference Table

| Flag / Option | Description |
|---|---|
| `--user` | Install binary for current user (Default mode). |
| `--system` | Install binary system-wide (Requires Administrator / root privileges). |
| `-d, --dir <DIR>` | Custom destination directory override for binary files. |
| `--service` | Install Linux `systemd` background service (`dvd-ripper.service`) and `udev` disc insertion rules (`99-dvd-ripper.rules`). |
| `-u, --uninstall` | Uninstall `dvd-ripper`, remove binaries, clean PATH, and remove services. |
| `-y, --yes` | Non-interactive mode (automatically answer yes to interactive prompts). |

---

## 3. Step-by-Step Installation Workflows

### 3.1 Standard User Installation (Linux / macOS / Windows)

```bash
# 1. Build release binaries
cargo build --release

# 2. Run user-level installer
cargo run --bin dvd-ripper-installer
```

### 3.2 System-Wide Installation with Linux Systemd Appliance Service

```bash
# Run system-wide installer with background daemon service and udev rules
sudo target/release/dvd-ripper-installer --system --service -y
```

---

## 4. Automated Dependency & System Audit

During execution, `dvd-ripper-installer`:

1. **FFmpeg Auditing**: Checks if `ffmpeg` is available in system `PATH` and verifies version compatibility. Reports warnings if FFmpeg lacks `dvdvideo` demuxer support.
2. **Binary Copying**: Locates compiled release binary (`dvd-ripper`) and copies it to the target installation directory.
3. **PATH Management**:
   - **Windows**: Updates User or System `PATH` registry environment keys (`HKCU\Environment` or `HKLM\System\CurrentControlSet\Control\Session Manager\Environment`).
   - **Linux / macOS**: Appends export lines to `~/.bashrc`, `~/.zshrc`, or updates `/etc/profile.d/dvd-ripper.sh`.

---

## 5. Linux Systemd Service & Udev Rules Integration

When `--service` is specified on Linux system-wide installs:

### 5.1 Systemd Unit File Installation (`contrib/dvd-ripper.service`)
Copies unit file to `/etc/systemd/system/dvd-ripper.service`:

```ini
[Unit]
Description=DVD Ripper Appliance Daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/dvd-ripper --daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 5.2 Udev Disc Insertion Auto-Trigger (`contrib/99-dvd-ripper.rules`)
Copies udev rules to `/etc/udev/rules.d/99-dvd-ripper.rules`:

```udev
# Trigger dvd-ripper daemon scan on optical disc insertion
ACTION=="change", SUBSYSTEM=="block", KERNEL=="sr*", ENV{ID_CDROM_MEDIA_DVD}=="1", TAG+="systemd", ENV{SYSTEMD_WANTS}="dvd-ripper.service"
```

---

## 6. Uninstallation Procedure

To completely remove `dvd-ripper` from your system:

```bash
# Run uninstaller
dvd-ripper-installer --uninstall -y
```

The uninstaller:
- Deletes installed binary files from installation directory.
- Restores original `PATH` environment variables.
- Stops and disables `dvd-ripper.service` systemd daemon (on Linux).
- Removes `/etc/udev/rules.d/99-dvd-ripper.rules` (on Linux).
