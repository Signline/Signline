#!/usr/bin/env bash
# ==============================================================================
# Signline v0.1.0b (Beta) — Debian (.deb) Installer Script
# ==============================================================================
# Compiles signline_0.1.0b_<arch>.deb from source (if not already present in
# build_script/dist/) and installs it onto the system via dpkg, launching the interactive
# Ratatui TUI installer (/usr/bin/signline-installer).
# ==============================================================================

set -euo pipefail

VERSION="0.1.0b"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEB_FILE="${SCRIPT_DIR}/dist/signline_${VERSION}_${ARCH}.deb"

if [ ! -f "${DEB_FILE}" ] || [ "${1:-}" = "--rebuild" ]; then
  echo "[*] Building Signline v${VERSION} Debian package from source..."
  "${SCRIPT_DIR}/build_deb.sh"
fi

echo ""
echo "[*] Installing ${DEB_FILE}..."
sudo dpkg -i "${DEB_FILE}" || sudo apt-get install -f -y
