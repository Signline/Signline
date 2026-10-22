#!/usr/bin/env bash
# ==============================================================================
# Signline v0.1.0b (Beta) Automated Build & Debian (.deb) Packaging Script
# ==============================================================================
# Stack:
#   - Backend:   Rust (Axum + Tokio)
#   - Database:  PocketBase (embedded daemon + 34 domain collections)
#   - Frontend:  TypeScript + React 18 + Vite + TailwindCSS + ShadCN/UI
#   - Installer: Ratatui Interactive TUI Installer (signline-installer)
#
# Usage:
#   ./build_script/build_deb.sh              # Compile & package signline_0.1.0b_amd64.deb
#   ./build_script/build_deb.sh --install    # Compile, package, and install via dpkg -i
#
# Attribution:
#   Powered by Signline, based on Signline (https://github.com/signline/signline)
#   Distributed under the GNU AGPLv3 License with Section 7(b) Additional Terms.
# ==============================================================================

set -euo pipefail

VERSION="0.1.0b"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/../VERSION" ] || [ -f "${SCRIPT_DIR}/../.version" ]; then
  PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
elif [ -n "${SIGNLINE_PROJECT_ROOT:-}" ]; then
  PROJECT_ROOT="${SIGNLINE_PROJECT_ROOT}"
else
  echo "Error: project root not found; run this script from build_script/ or set SIGNLINE_PROJECT_ROOT" >&2
  exit 1
fi

DEB_DIST_DIR="${SCRIPT_DIR}/dist"
ROOT_DIST_DIR="${PROJECT_ROOT}/dist"

echo "=============================================================================="
echo " Signline v${VERSION} — Automated Dependency Check, Compiler & .deb Builder"
echo "=============================================================================="

# ------------------------------------------------------------------------------
# 1. Check & Install Required System Dependencies (Requests sudo if missing)
# ------------------------------------------------------------------------------
MISSING_APT_PACKAGES=()

check_cmd_pkg() {
  local cmd="$1"
  local pkg="$2"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    MISSING_APT_PACKAGES+=("${pkg}")
  fi
}

check_cmd_pkg "gcc" "build-essential"
check_cmd_pkg "pkg-config" "pkg-config"
check_cmd_pkg "curl" "curl"
check_cmd_pkg "unzip" "unzip"
check_cmd_pkg "dpkg-deb" "dpkg-dev"
check_cmd_pkg "pdftoppm" "poppler-utils"
check_cmd_pkg "node" "nodejs"
check_cmd_pkg "npm" "npm"

if ! dpkg -s libssl-dev >/dev/null 2>&1; then
  MISSING_APT_PACKAGES+=("libssl-dev")
fi

if [ "${#MISSING_APT_PACKAGES[@]}" -gt 0 ]; then
  echo "[!] Missing required build dependencies: ${MISSING_APT_PACKAGES[*]}"
  echo "[*] Requesting sudo privileges to install missing packages..."
  sudo apt-get update
  sudo apt-get install -y "${MISSING_APT_PACKAGES[@]}"
else
  echo "[✓] All required system packages (build-essential, pkg-config, libssl-dev, curl, unzip, dpkg-dev, poppler-utils, nodejs, npm) are present."
fi

# Check Rust / Cargo toolchain
if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
  echo "[!] Rust toolchain (cargo/rustc) not found. Installing via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1090
  source "${HOME}/.cargo/env"
else
  echo "[✓] Rust toolchain found: $(rustc --version)"
fi

# Ensure PocketBase binary is present
PB_BIN="${PROJECT_ROOT}/bin/pocketbase"
if [ ! -x "${PB_BIN}" ]; then
  echo "[*] Downloading PocketBase v0.25.9 binary..."
  mkdir -p "${PROJECT_ROOT}/bin"
  curl -sL "https://github.com/pocketbase/pocketbase/releases/download/v0.25.9/pocketbase_0.25.9_linux_amd64.zip" -o /tmp/pb_build.zip
  unzip -o /tmp/pb_build.zip pocketbase -d "${PROJECT_ROOT}/bin/"
  chmod +x "${PB_BIN}"
  rm -f /tmp/pb_build.zip
fi
echo "[✓] PocketBase binary verified: $(${PB_BIN} --version)"

# ------------------------------------------------------------------------------
# 2. Build Frontend (TypeScript + React 18 + ShadCN/UI)
# ------------------------------------------------------------------------------
echo ""
echo "[1/4] Building TypeScript + ShadCN/UI Frontend..."
pushd "${PROJECT_ROOT}/frontend" >/dev/null
if [ ! -d "node_modules" ]; then
  npm install
fi
npm run build
popd >/dev/null
echo "[✓] Frontend production bundle built in ${PROJECT_ROOT}/frontend/dist"

# ------------------------------------------------------------------------------
# 3. Build Rust Backend (signline-backend) & Ratatui TUI Installer (signline-installer)
# ------------------------------------------------------------------------------
PROFILE_FLAG="--release"
TARGET_SUBDIR="release"
if [ "${SIGNLINE_FAST_BUILD:-0}" = "1" ]; then
  PROFILE_FLAG=""
  TARGET_SUBDIR="debug"
fi

echo ""
echo "[2/4] Compiling Rust Backend (signline-backend)..."
cargo build ${PROFILE_FLAG} --manifest-path "${PROJECT_ROOT}/backend/Cargo.toml"
BACKEND_BIN="${PROJECT_ROOT}/backend/target/${TARGET_SUBDIR}/signline-backend"
echo "[✓] Compiled backend binary: ${BACKEND_BIN}"

echo ""
echo "[3/4] Compiling Ratatui TUI Installer (signline-installer)..."
TUI_DIR="${SCRIPT_DIR}/tui-installer"
cargo build ${PROFILE_FLAG} --manifest-path "${TUI_DIR}/Cargo.toml"
INSTALLER_BIN="${TUI_DIR}/target/${TARGET_SUBDIR}/signline-installer"
echo "[✓] Compiled Ratatui TUI installer binary: ${INSTALLER_BIN}"

# ------------------------------------------------------------------------------
# 4. Assemble & Build Debian Package (.deb)
# ------------------------------------------------------------------------------
echo ""
echo "[4/4] Packaging Signline v${VERSION} into Debian (.deb) archive..."
STAGE_DIR="$(mktemp -d /tmp/signline-deb-stage.XXXXXX)"
trap 'rm -rf "${STAGE_DIR}"' EXIT

mkdir -p \
  "${STAGE_DIR}/DEBIAN" \
  "${STAGE_DIR}/usr/bin" \
  "${STAGE_DIR}/usr/lib/signline/bin" \
  "${STAGE_DIR}/usr/share/signline/public" \
  "${STAGE_DIR}/usr/share/signline/frontend/dist" \
  "${STAGE_DIR}/usr/share/doc/signline" \
  "${STAGE_DIR}/lib/systemd/system" \
  "${STAGE_DIR}/etc/signline" \
  "${STAGE_DIR}/var/lib/signline/pb_data" \
  "${STAGE_DIR}/var/lib/signline/storage"

# Copy binaries
cp "${BACKEND_BIN}" "${STAGE_DIR}/usr/bin/signline-server"
chmod 0755 "${STAGE_DIR}/usr/bin/signline-server"

cp "${INSTALLER_BIN}" "${STAGE_DIR}/usr/bin/signline"
chmod 0755 "${STAGE_DIR}/usr/bin/signline"

cp "${INSTALLER_BIN}" "${STAGE_DIR}/usr/bin/signline-installer"
chmod 0755 "${STAGE_DIR}/usr/bin/signline-installer"

cp "${PB_BIN}" "${STAGE_DIR}/usr/lib/signline/bin/pocketbase"
chmod 0755 "${STAGE_DIR}/usr/lib/signline/bin/pocketbase"

# Copy static assets & compiled frontend SPA
cp -r "${PROJECT_ROOT}/public/"* "${STAGE_DIR}/usr/share/signline/public/"
cp -r "${PROJECT_ROOT}/frontend/dist/"* "${STAGE_DIR}/usr/share/signline/frontend/dist/"
cp "${PROJECT_ROOT}/LICENSE" "${PROJECT_ROOT}/LICENSE_ADDITIONAL_TERMS" "${PROJECT_ROOT}/README.md" "${PROJECT_ROOT}/CHANGELOG.md" "${STAGE_DIR}/usr/share/doc/signline/"

# Create systemd service unit
cat > "${STAGE_DIR}/lib/systemd/system/signline.service" <<'EOF'
[Unit]
Description=Signline Open Source Digital Signature Platform (Rust + PocketBase)
Documentation=https://github.com/signline/signline
After=network.target

[Service]
Type=simple
EnvironmentFile=-/etc/signline/signline.env
WorkingDirectory=/var/lib/signline
ExecStart=/usr/bin/signline-server
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# Create DEBIAN/control
INSTALLED_SIZE="$(du -sk "${STAGE_DIR}/usr" | awk '{print $1}')"
cat > "${STAGE_DIR}/DEBIAN/control" <<EOF
Package: signline
Version: ${VERSION}
Section: web
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Depends: libc6, libssl3 | libssl1.1, poppler-utils, ca-certificates
Maintainer: c0fe <c0fe@users.noreply.github.com>
Homepage: https://github.com/signline/signline
Description: Open Source Digital Signature Platform (Rust + PocketBase + ShadCN/UI)
 Signline is a full-featured Open Source Digital Signature Platform built
 in Rust, PocketBase, and TypeScript (React 18 + ShadCN/UI). Includes an interactive
 Ratatui TUI manager and installer (/usr/bin/signline and /usr/bin/signline-installer).
 .
 Powered by Signline, based on Signline (AGPLv3 Section 7(b)).
EOF

# Create DEBIAN/postinst that invokes the Ratatui TUI installer (signline)
cat > "${STAGE_DIR}/DEBIAN/postinst" <<'EOF'
#!/usr/bin/env bash
set -e

mkdir -p /etc/signline /var/lib/signline/pb_data /var/lib/signline/storage

if [ "$1" = "configure" ]; then
  if [ -t 0 ] && [ -t 1 ] && [ "${DEBIAN_FRONTEND:-}" != "noninteractive" ]; then
    echo "Launching Signline Ratatui TUI Installer..."
    /usr/bin/signline --wizard --env-file /etc/signline/signline.env || true
  else
    /usr/bin/signline --non-interactive --env-file /etc/signline/signline.env || true
  fi
  if [ -d /run/systemd/system ]; then
    systemctl daemon-reload >/dev/null 2>&1 || true
    systemctl enable signline.service >/dev/null 2>&1 || true
    systemctl restart signline.service >/dev/null 2>&1 || true
  fi
fi

exit 0
EOF
chmod 0755 "${STAGE_DIR}/DEBIAN/postinst"

# Create DEBIAN/prerm
cat > "${STAGE_DIR}/DEBIAN/prerm" <<'EOF'
#!/usr/bin/env bash
set -e
if [ "$1" = "remove" ] && [ -d /run/systemd/system ]; then
  systemctl stop signline.service >/dev/null 2>&1 || true
  systemctl disable signline.service >/dev/null 2>&1 || true
fi
exit 0
EOF
chmod 0755 "${STAGE_DIR}/DEBIAN/prerm"

mkdir -p "${DEB_DIST_DIR}"
DEB_FILENAME="signline_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "${STAGE_DIR}" "${DEB_DIST_DIR}/${DEB_FILENAME}"

echo ""
echo "=============================================================================="
echo " [SUCCESS] Built Debian Package & Ratatui TUI Manager/Installer!"
echo "   Package Path : ${DEB_DIST_DIR}/${DEB_FILENAME}"
echo "   TUI CLI      : /usr/bin/signline (and /usr/bin/signline-installer)"
echo "   Server Binary: /usr/bin/signline-server (bundled in .deb)"
echo "   To Install   : sudo dpkg -i ${DEB_DIST_DIR}/${DEB_FILENAME}"
echo "=============================================================================="

if [ "${1:-}" = "--install" ]; then
  echo ""
  echo "[*] Installing ${DEB_DIST_DIR}/${DEB_FILENAME} via dpkg..."
  sudo dpkg -i "${DEB_DIST_DIR}/${DEB_FILENAME}" || sudo apt-get install -f -y
fi
