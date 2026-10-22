# Changelog

## 0.1.0b (Beta Pre-Release)

First beta pre-release of **Signline** (`0.1.0b` — the appended `b` denotes a **beta** pre-release), built on a modern **Rust + PocketBase + TypeScript (React 18 / ShadCN/UI)** stack with an interactive **Ratatui TUI Installer** and automated Debian (`.deb`) packaging.

### Architecture & Tech Stack (`0.1.0b`)

- **Rust Backend (`backend/`)**: Built with Axum and Tokio. Organized into modular, human-friendly domain packages (`pb`, `models`, `auth`, `pdf`, `crypto`, `services`, `routes`).
- **PocketBase Persistence (`bin/pocketbase` + `pb_data/`)**: 34 managed PocketBase collections automatically provisioned and migrated by the Rust server on boot.
- **TypeScript + React 18 + ShadCN/UI Frontend (`frontend/`)**: A modular TypeScript SPA using ShadCN/UI components, TailwindCSS, and dynamic CSS custom properties.
- **Ratatui TUI Installer & Automated `.deb` Builder (`build_script/`, `build_script/tui-installer/`)**: Includes `signline-installer` (built with `ratatui` + `crossterm`) for interactive terminal-based configuration and `build_script/build_deb.sh` to verify/install dependencies, compile all artifacts, and package `signline_0.1.0b_amd64.deb`.

### Features Included in `0.1.0b`

- **WYSIWYG PDF & HTML Template Builder**: Interactive multi-page canvas with zoom, drag-and-drop field placement and resizing, 20 field types (`signature`, `initials`, `text`, `date`, `datestamp`, `timestamp`, `checkbox`, `radio`, `select`, `multiple`, `cells`, `number`, `phone`, `file`, `image`, `payment`, `verification`, `kba`, `heading`, `note`), multi-party submitter roles, conditional rules, formulas, auto-field detection, and dynamic HTML-to-PDF templates.
- **Multi-Party Guided Document Signing (`/s/:slug` & `/d/:slug`)**: Sequential (`preserved`) or parallel (`random`) signing order, guided field wizard, HTML5 Canvas drawn signatures, typed script signatures, image uploads, mobile QR code signing, email 2FA OTP verification, signing delegation (`/s/:slug/delegate`), and decline flow (`/s/:slug/decline`).
- **Cryptographic X.509 / PKCS#7 PDF Digital Signatures & Verification (`/verify`)**: Automatic 2048-bit RSA + X.509 certificate provisioning (`O=Signline`), custom PKCS#12 (`.p12`) certificate upload, detached PKCS#7 (`/ByteRange` + `/Contents`) PDF signing, optional RFC 3161 Timestamp Authority (TSA) integration, SHA-256 `completed_documents` registry, Audit Trail Certificate PDF generation, and public/API cryptographic PDF signature verifier.
- **White-Label Personalization (`/settings/personalization`)**: Custom product name, primary and accent hex colors with automatic luminance contrast calculation (`0.62` threshold), custom logo and favicon uploads, email sender name and footer, and support/legal links.
- **AGPLv3 Section 7(b) Attribution**: Persistent `"based on Signline"` credit linking to `https://github.com/signline/signline` across all web layouts, signing pages, and outgoing emails, unaffected by white-label personalization settings.
- **Gmail OAuth 2.0, RFC 2822 MIME PDF Attachments & SMTP Delivery (`/settings/email`)**:
  - Connect Gmail or Google Workspace via the OAuth 2.0 authorization code flow (`gmail.send` scope) with a single-use `state` nonce, PKCE (`S256`), automatic token refresh and precedence over SMTP.
  - Full `multipart/mixed` RFC 2822 MIME message builder attaching Base64-encoded PDFs for signature requests (`submitter_invitation`, `submitter_invitation_resend`, `submitter_delegated`) and attaching both the signed PDF (`-signed.pdf`) and Audit Trail Certificate PDF (`-audit-trail.pdf`) upon completion (`submission_completed`).
  - Complete transactional email coverage including `welcome_email`, `password_reset`, `password_reset_confirmed`, `submission_declined`, `submitter_email_2fa`, `submitter_invitation_reminder`, and `test_email`.
- **Automated Signature Request Reminders (`/settings/notifications`)**: Configurable 1st, 2nd, and 3rd reminder schedules (from 1 hour up to 30 days) for pending recipients.
- **Webhooks, REST API & MCP Server (`/settings/webhooks`, `/settings/api`, `/settings/mcp`, `/mcp`)**: Full Signline REST API (`X-Auth-Token` and `X-Account-Testing`), HMAC-SHA256 signed webhooks (`X-Signline-Signature`), and Model Context Protocol (MCP) JSON-RPC 2.0 server with 7 tools.
