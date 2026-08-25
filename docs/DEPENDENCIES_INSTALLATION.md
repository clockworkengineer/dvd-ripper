# FFmpeg & libdvdcss Installation Guide

This guide provides step-by-step instructions for installing **FFmpeg** and **libdvdcss** (the CSS decryption library required for encrypted commercial DVDs) across **macOS**, **Linux**, and **Windows**.

---

## 📋 Overview

DVD Ripper relies on **FFmpeg** to probe optical titles, extract audio/video streams, and transcode video. For commercial encrypted DVDs, FFmpeg requires **libdvdcss** to handle Content Scramble System (CSS) decryption.

---

## 🍏 macOS Installation

### Option 1: Using Homebrew (Recommended)

Homebrew installs both FFmpeg and libdvdcss with full decryption support:

```bash
# 1. Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 2. Install FFmpeg and libdvdcss
brew install ffmpeg libdvdcss
```

### Verification on macOS
```bash
ffmpeg -version
ls -la /opt/homebrew/lib/libdvdcss*   # Apple Silicon (M1/M2/M3)
# or
ls -la /usr/local/lib/libdvdcss*      # Intel Macs
```

---

## 🐧 Linux Installation

### Ubuntu / Debian / Linux Mint

```bash
# Update package repositories
sudo apt update

# Install FFmpeg and DVD access libraries
sudo apt install -y ffmpeg libdvd-pkg

# Build and configure libdvdcss
sudo dpkg-reconfigure libdvd-pkg
```

### Fedora / RHEL / AlmaLinux / Rocky Linux

Enable RPM Fusion to access FFmpeg and libdvdcss:

```bash
# Enable RPM Fusion free repository
sudo dnf install -y https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm

# Install FFmpeg and libdvdcss
sudo dnf install -y ffmpeg libdvdcss
```

### Arch Linux / Manjaro

```bash
# Install FFmpeg and libdvdcss from official repositories
sudo pacman -S --needed ffmpeg libdvdcss
```

---

## 🪟 Windows Installation

### Option 1: Using Chocolatey (Recommended)

```powershell
# Run PowerShell as Administrator
choco install ffmpeg libdvdcss -y
```

### Option 2: Using Scoop

```powershell
# Open PowerShell
scoop bucket add main
scoop install ffmpeg libdvdcss
```

### Option 3: Using Winget (Windows Package Manager)

```powershell
winget install Gyan.FFmpeg
```
> **Note**: When installing FFmpeg via Winget or manual zip downloads, obtain `libdvdcss-2.dll` from Videolan/VLC or download a precompiled `libdvdcss-2.dll` and place it in your `C:\Windows\System32` directory or alongside `ffmpeg.exe` in your system `PATH`.

---

## 🔍 Verification & Troubleshooting

To verify that FFmpeg is properly recognized by DVD Ripper:

```bash
# Check FFmpeg CLI availability
ffmpeg -version

# Run DVD Ripper drive detection & health check
dvd-ripper --cli --input D:\
```

If DVD Ripper reports CSS decryption errors:
1. Ensure `libdvdcss` (or `libdvdcss-2.dll`) is installed in your system library path or executable folder.
2. Verify optical disc drive read permissions:
   - **Linux**: `sudo usermod -aG cdrom,optical $USER`
   - **macOS**: Grant "Full Disk Access" to Terminal/DVD Ripper in *System Settings > Privacy & Security*.
