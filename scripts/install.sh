#!/usr/bin/env bash
# Standalone quick installer for dvd-ripper on Linux and macOS
set -euo pipefail

REPO="clockworkengineer/dvd-ripper"
INSTALL_DIR="${HOME}/.local/bin"

echo "===================================================="
echo "   DVD Ripper Standalone Unix Quick Installer       "
echo "===================================================="

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux)
        case "${ARCH}" in
            x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
            aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "Unsupported Linux architecture: ${ARCH}"; exit 1 ;;
        esac
        ;;
    Darwin)
        case "${ARCH}" in
            x86_64) TARGET="x86_64-apple-darwin" ;;
            arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
            *) echo "Unsupported macOS architecture: ${ARCH}"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported operating system: ${OS}. On Windows, use the installer executable or WinGet."
        exit 1
        ;;
esac

ASSET_NAME="dvd-ripper-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"

echo "[+] Target platform: ${TARGET}"
echo "[+] Destination directory: ${INSTALL_DIR}"
mkdir -p "${INSTALL_DIR}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "[+] Fetching latest release asset: ${DOWNLOAD_URL}..."
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "${DOWNLOAD_URL}" -o "${TMP_DIR}/${ASSET_NAME}"
elif command -v wget >/dev/null 2>&1; then
    wget -q "${DOWNLOAD_URL}" -O "${TMP_DIR}/${ASSET_NAME}"
else
    echo "Error: neither curl nor wget found."
    exit 1
fi

echo "[+] Extracting archive..."
tar -xzf "${TMP_DIR}/${ASSET_NAME}" -C "${TMP_DIR}"

# Locate extracted binary
if [ -f "${TMP_DIR}/dvd-ripper" ]; then
    cp "${TMP_DIR}/dvd-ripper" "${INSTALL_DIR}/dvd-ripper"
    chmod 755 "${INSTALL_DIR}/dvd-ripper"
    echo "[+] Installed ${INSTALL_DIR}/dvd-ripper"
fi

if [ -f "${TMP_DIR}/dvd-ripper-installer" ]; then
    cp "${TMP_DIR}/dvd-ripper-installer" "${INSTALL_DIR}/dvd-ripper-installer"
    chmod 755 "${INSTALL_DIR}/dvd-ripper-installer"
fi

# Ensure ~/.local/bin is in PATH
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo "[!] Notice: ${INSTALL_DIR} is not currently in your PATH."
    echo "    Add the following line to your ~/.bashrc or ~/.zshrc:"
    echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo ""
echo "Installation completed! Run 'dvd-ripper --help' to get started."
