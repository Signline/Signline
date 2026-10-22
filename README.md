<h1 align="center" style="border-bottom: none">
  <img alt="Signline" src="public/logo-long.svg" width="360" />
</h1>
<h3 align="center">
  Open source document filling and signing — Rust + PocketBase + TypeScript (ShadCN/UI) Edition (v0.1.0b Beta)
</h3>

Signline is an open source platform that provides secure and efficient digital document signing and processing. Create PDF and HTML forms to have them filled and signed online on any device with an easy-to-use, mobile-optimized web interface.

Signline is an open-source electronic signing platform — see [Attribution](#attribution).

> **Release `0.1.0b` (Beta Pre-Release)**: The appended `b` in `0.1.0b` denotes the first **beta** pre-release of Signline (`Rust` Axum/Tokio backend + `PocketBase` persistence + `TypeScript` React 18 / ShadCN/UI frontend + `Ratatui` TUI installer). See [CHANGELOG.md](CHANGELOG.md) for full details.

---

## Features

- **WYSIWYG PDF & Dynamic HTML Form Builder**: Drag, resize, and configure 20 field types (`signature`, `initials`, `text`, `date`, `checkbox`, `radio`, `select`, `multiple`, `cells`, `file`, `image`, `number`, `phone`, `verification`, `kba`, `payment`, `datestamp`, `timestamp`, `heading`, `note`) with conditional logic and formulas.
- **Multi-Party Signing Workflows**: Sequential (`preserved`) or parallel (`random`) signing order, email 2FA verification, signing delegation, and decline workflows.
- **Cryptographic X.509 / PKCS#7 PDF Signing & Audit Trails**: Automatic 2048-bit RSA X.509 certificate generation, custom PKCS#12 (`.p12`) upload, detached PKCS#7 `/ByteRange` digital signatures, RFC 3161 Timestamp Authority (TSA) support, tamper-evident signature ID stamps, Audit Trail Certificate PDFs, and built-in PDF signature verification (`/verify`).
- **Automated Emails via Gmail OAuth 2.0 (with PDF Attachments) or SMTP**: Connect a Gmail or Google Workspace mailbox via the OAuth 2.0 authorization code flow with PKCE (`gmail.send` scope), or use standard SMTP. Gmail automatically takes precedence while connected and sends RFC 2822 `multipart/mixed` emails with PDF attachments for signature invitations and completed signed/audit-trail PDFs.
- **Automated Signature Request Reminders**: Schedule up to 3 automatic follow-up reminders (1 hour to 30 days) for recipients who have not yet completed their documents.
- **White-Label Personalization**: Customize product name, logo, favicon, primary & accent brand colors (with automatic HSL contrast calculation), email sender name & footer, and support/legal links in **Settings -> Personalization**.
- **PocketBase Persistence & Storage**: All 34 domain collections are automatically provisioned and managed in PocketBase (`pb_data/`), with support for local disk, AWS S3, Google Cloud Storage, and Azure Blob storage.
- **REST API, Webhooks & MCP Server**: Compatible with Signline API clients (`X-Auth-Token`), `X-Signline-Signature` HMAC-SHA256 webhooks, and Model Context Protocol (`POST /mcp`).
- **Two Dedicated Packaging & Deployment Directories (`build_script/` and `docker/`)**:
  - **[`build_script/`](build_script/README.md)**: Native Debian (`.deb`) compiler, packager, and installer bundling `signline-server`, `pocketbase`, and the interactive **Ratatui TUI Installer** (`signline-installer`).
  - **[`docker/`](docker/README.md)**: Optional multi-stage `Dockerfile` and `docker-compose.yml` for containerized deployments.

---

## Directory Structure

```text
signline/
├── .version                 # Release version ("0.1.0b")
├── VERSION                  # Release version ("0.1.0b")
├── backend/                 # Modular Rust (Axum + Tokio) server & PDF/Crypto engine
├── frontend/                # TypeScript + React 18 + TailwindCSS + ShadCN/UI SPA
├── build_script/            # Native Debian (.deb) compiler, installer & Ratatui TUI wizard
│   ├── build_deb.sh         # Checks dependencies (with sudo if needed), compiles & builds .deb
│   ├── install_deb.sh       # Compiles (if needed) and installs signline_0.1.0b_amd64.deb via dpkg
│   ├── tui-installer/       # Ratatui terminal UI installer crate (/usr/bin/signline-installer)
│   ├── dist/                # Compiled .deb package output (signline_0.1.0b_amd64.deb)
│   └── README.md            # Debian (.deb) build & installation guide
├── docker/                  # Optional Docker & Docker Compose deployment configuration
│   ├── Dockerfile           # Multi-stage Dockerfile
│   ├── docker-compose.yml   # Docker Compose service configuration
│   ├── .dockerignore        # Docker build context ignore rules
│   └── README.md            # Docker deployment guide
├── bin/
│   └── pocketbase           # Embedded PocketBase server binary
├── public/                  # Brand logos, favicons, and static assets
└── pb_data/                 # PocketBase database & storage directory (created at runtime)
```

---

## Installation & Deployment Options

### Option 1: Native Debian (`.deb`) Package + Ratatui TUI Installer (`build_script/`)
Compile the `.deb` package from source and install it onto Debian/Ubuntu:

```bash
# 1. Compile and build build_script/dist/signline_0.1.0b_amd64.deb
./build_script/build_deb.sh

# 2. Install the .deb package (automatically launches the Ratatui TUI installer)
sudo dpkg -i build_script/dist/signline_0.1.0b_amd64.deb
```
Or run `./build_script/install_deb.sh` to build and install in a single step. See [`build_script/README.md`](build_script/README.md) for full details.

### Option 2: Optional Docker Deployment (`docker/`)
```bash
docker compose -f docker/docker-compose.yml up --build -d
```
See [`docker/README.md`](docker/README.md) for full details.

### Option 3: Run Directly from Source
```bash
# 1. Build the Frontend
cd frontend && npm install && npm run build && cd ..

# 2. Run the Rust Backend (Auto-starts & Migrates PocketBase)
cd backend && cargo run --release
```

By default:
- The **Signline Web UI, REST API & MCP Server** listens on `http://127.0.0.1:3000`.
- The managed **PocketBase Server** runs on `http://127.0.0.1:8090` (with data persisted in `pb_data/`).

---

## Attribution

Signline is an open-source document signing platform (`https://github.com/signline/signline`).

- Project: https://github.com/signline/signline

Signline is distributed under the GNU AGPLv3 with additional terms under Section 7(b) which require the original Signline attribution to be retained in interactive user interfaces. Every interface that carries the product name also carries a `"based on Signline"` credit linking to the project, and that credit cannot be removed through the white-label personalization settings.

## License

Distributed under the AGPLv3 License with Section 7(b) Additional Terms. See [LICENSE](LICENSE) and [LICENSE_ADDITIONAL_TERMS](LICENSE_ADDITIONAL_TERMS) for more information.
