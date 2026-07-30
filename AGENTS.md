# AGENTS.md — Agenfact Coding Agent Guide

> **Purpose**: This document is optimized for AI coding agents (Claude Code, Codex, etc.) to quickly understand the codebase structure, conventions, and common modification patterns.

---

## 🎯 Quick Reference

| Aspect | Value |
|--------|-------|
| **Type** | Self-hosted stateless digital artifact delivery & hosting platform for AI Agents |
| **Backend** | Rust 2024 edition, Actix Web 4 |
| **Frontend** | Svelte 5, Vite, TypeScript, Tailwind CSS 4 |
| **Auth** | Cloudflare Access JWT (RS256/JWKS or HS256) |
| **Storage** | Local filesystem + JSON indices (with optional CouchDB backend) |
| **Config** | Figment (TOML + env vars with `AGENFACT_` prefix) |
| **CI/CD** | Gitea Actions (`.gitea/workflows/`) |

---

## 📁 Project Structure

```
agenfact/
├── src/                          # Rust backend
│   ├── main.rs                   # Entry point, route mounting, managed state
│   ├── config.rs                 # Figment config (TOML + env), path normalization
│   ├── auth.rs                   # JWT validation (RS256/JWKS + HS256), VerifiedIdentity guard
│   ├── error.rs                  # Unified AgenfactError → JSON HTTP error responses
│   ├── files.rs                  # File CRUD (GET/POST/PUT/DELETE), SafePath validation
│   ├── uploads.rs                # Random 8-char filename, multipart upload, TTL scheduling
│   ├── expiry.rs                 # Background sweeper (60s interval), ExpiryStore
│   ├── path.rs                   # SafePath validation for user-supplied paths
│   ├── private_index.rs          # Private file authorization, PrivateIndexStore
│   ├── couchdb.rs                # CouchDB persistence & 60s background sweeper
│   ├── upload_protection.rs      # Turnstile challenge, rate limiter, disk check
│   ├── store.rs                  # Generic mutex-protected JSON store with atomic writes
│   └── test_utils.rs             # Test helpers (#[cfg(test)])
├── web/                          # Svelte frontend
│   ├── src/
│   │   ├── App.svelte            # Main upload UI
│   │   ├── components/
│   │   │   ├── FileUploadZone.svelte   # Drag & drop upload + laser sweep
│   │   │   ├── DownloadLink.svelte     # Short URL display + copy
│   │   │   ├── UploadOptions.svelte    # Email ACL & TTL settings
│   │   │   ├── ArtifactPreviewModal.svelte # Markdown/Code/Media viewer
│   │   │   ├── QRCodeModal.svelte      # Mobile share QR code
│   │   │   └── TurnstileWidget.svelte  # Cloudflare Turnstile challenge widget
│   │   ├── main.ts               # Entry point (Svelte mount)
│   │   ├── app.css               # Tailwind CSS imports & cyber styles
│   │   └── app.d.ts              # Svelte type declarations
│   ├── package.json              # pnpm install, pnpm dev, pnpm run build
│   ├── vite.config.ts            # Vite + Svelte + Tailwind config
│   ├── svelte.config.js          # Svelte preprocessor config
│   ├── theme.css                 # Theme CSS custom properties
│   ├── tokens.json               # Design Tokens specification
│   └── DESIGN.md                 # UI design system specification
├── scripts/                      # Utility scripts
│   └── migrate_to_couchdb.py     # One-time migration script from JSON to CouchDB
├── skills/                       # AI Agent Skills
│   └── agenfact/                 # Agenfact skill for AI agents (upload.py, compress.py)
├── data/                         # Runtime data (created at runtime)
│   ├── expiry-index.json         # File expiration tracking
│   └── private-files.json        # Private file authorization lists
├── uploads/                      # Uploaded files (created at runtime)
├── .gitea/workflows/             # CI/CD pipelines
│   ├── rust.yml                  # Build + test + Trivy scan
│   └── docker.yml                # Docker build + Trivy + push
├── docker-compose.yml            # Docker Compose configuration (App + CouchDB)
├── Dockerfile                    # Multi-stage: node/pnpm (web) + rust (backend) → debian
└── Cargo.toml                    # Dependencies: actix-web, actix-files, actix-multipart, figment, jsonwebtoken, reqwest
```

---

## 🚀 Routes & Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET` | `/health` | `health()` | Health check → "OK" |
| `POST` | `/uploads` | `uploads::upload_file()` | Random ID upload + TTL + private marking |
| `GET` | `/files/<path>` | `files::get_file()` | Public download (redirects if private) |
| `POST` | `/files/<path>` | `files::create_file()` | Create at explicit path (409 if exists) |
| `PUT` | `/files/<path>` | `files::upsert_file()` | Create or overwrite |
| `DELETE` | `/files/<path>` | `files::delete_file()` | Delete file |
| `GET` | `/private-files/<path>` | `files::get_private_file()` | JWT-protected download |
| `GET` | `/` | `FileServer` | Serve Svelte SPA static assets |

---

## 🔐 Security Model

### Path Normalization (Defense in Depth)

1. **`SafePath` validation** (`path.rs` + `files.rs`): Rejects `..` and non-normal URL path components before filesystem access
2. **`config::Agenfact::normalize_and_join()`** (`config.rs`): Strips `..`, `.`, root components; only `Normal` components kept

**When modifying**: Always use `build_full_upload_path()` or `build_full_data_path()` — never join paths manually.

### JWT Authentication

- **Request Helper**: `VerifiedIdentity::from_request()` extracts and verifies tokens from Actix `HttpRequest`
- **Token Sources**: `Cf-Access-Jwt-Assertion` header (priority) OR `Authorization: Bearer ***`
- **Verify Modes**:
  - **RS256 + JWKS**: Production. Fetches from Cloudflare, caches 1hr in `Mutex<Option<(JwkSet, Instant)>>`
  - **HS256**: Testing. Uses `AGENFACT_CF_ACCESS_HS256_SECRET` env var

**When modifying auth**: Update both `verify_claims()` branches (RS256 + HS256).

### Write-Route Boundary

- `POST`, `PUT`, and `DELETE /files/<path>` do **not** apply `VerifiedIdentity` in the application.
- Production deployments must enforce their write-route policy at Cloudflare WAF/Access and must not expose the origin in a way that bypasses those controls.
- If application-layer write authentication is added, use the existing `VerifiedIdentity` flow and cover both RS256/JWKS and HS256 test modes.

### Private File Flow

```
GET /files/<path>
  ↓
private_index.is_private(path)?
  ├─ Yes → 302 Redirect → /private-files/<path>
  └─ No  → Serve file directly

GET /private-files/<path>
  ↓
VerifiedIdentity guard → AccessIdentity { sub, email }
  ↓
private_index.get_entry(path) → authorized_emails
  ↓
email ∈ authorized_emails?
  ├─ Yes → Serve file
  └─ No  → 403 Forbidden
```

---

## ⚙️ Configuration

### Core Settings (`config.rs`)

| Key | Env Var | Default | Description |
|-----|---------|---------|-------------|
| `web_path` | `AGENFACT_WEB_PATH` | `./web/dist` | Svelte build output path |
| `uploads_path` | `AGENFACT_UPLOADS_PATH` | `./uploads` | Uploaded files storage |
| `data_path` | `AGENFACT_DATA_PATH` | `./data` | JSON index files location |
| `default_upload_ttl_secs` | `AGENFACT_DEFAULT_UPLOAD_TTL_SECS` | `604800` | Positive default TTL when `expire` is omitted |
| `max_upload_ttl_secs` | `AGENFACT_MAX_UPLOAD_TTL_SECS` | `604800` | Positive maximum accepted TTL |

### CouchDB Settings (`config.rs`)

| Key | Env Var | Default | Description |
|-----|---------|---------|-------------|
| `couchdb_url` | `AGENFACT_COUCHDB_URL` | _(unset)_ | Base URL of CouchDB instance |
| `couchdb_db` | `AGENFACT_COUCHDB_DB` | `agenfact` | Target CouchDB database name |
| `couchdb_user` | `AGENFACT_COUCHDB_USER` | _(unset)_ | CouchDB Basic auth username |
| `couchdb_password` | `AGENFACT_COUCHDB_PASSWORD` | _(unset)_ | CouchDB Basic auth password |

### Cloudflare Access Settings (`auth.rs:from_env()`)

| Env Var | Default | Description |
|---------|---------|-------------|
| `AGENFACT_CF_ACCESS_ISSUER` | `https://example.cloudflareaccess.com` | JWT issuer |
| `AGENFACT_CF_ACCESS_AUD` | _(empty)_ | JWT audience (required for prod) |
| `AGENFACT_CF_ACCESS_JWKS_URL` | `${ISSUER}/cdn-cgi/access/certs` | JWKS endpoint |
| `AGENFACT_CF_ACCESS_HS256_SECRET` | _(unset)_ | HS256 secret (dev/test only) |

### Upload Protection Settings (`upload_protection.rs:from_env()`)

| Env Var | Default | Description |
|---------|---------|-------------|
| `AGENFACT_UPLOAD_RATE_SOFT_LIMIT` | `5` | Requests per window before Turnstile challenge; must not exceed the hard limit |
| `AGENFACT_UPLOAD_RATE_HARD_LIMIT` | `20` | Hard rate limit (`0..=100`; 429 when exceeded) |
| `AGENFACT_UPLOAD_RATE_WINDOW_SECS` | `60` | Time window for rate counting |
| `AGENFACT_TURNSTILE_PASS_TTL_SECS` | `600` | HttpOnly upload-pass cookie duration |
| `AGENFACT_MIN_FREE_DISK_BYTES` | `1073741824` | Minimum free disk space (1 GiB by default; 0 = disabled) |
| `AGENFACT_TRUST_CF_CONNECTING_IP` | `false` | Trust CF-Connecting-IP header (enable behind Cloudflare) |
| `AGENFACT_UPLOAD_TOKEN` | _(unset)_ | Bearer token for CLI uploads (bypasses Turnstile) |
| `AGENFACT_TURNSTILE_SITE_KEY` | _(unset)_ | Turnstile site key for frontend widget |
| `AGENFACT_TURNSTILE_SECRET` | _(unset)_ | Turnstile secret for server verification |
| `AGENFACT_TURNSTILE_HOSTNAME` | _(unset)_ | Expected hostname in Turnstile response |
| `AGENFACT_TURNSTILE_SITEVERIFY_URL` | `https://challenges.cloudflare.com/turnstile/v0/siteverify` | Turnstile API endpoint |
| `AGENFACT_MAX_CONCURRENT_UPLOADS` | `4` | System-wide concurrent upload limit |

The Turnstile site key, secret, and expected hostname are an all-or-none configuration set. Partial configuration is rejected at startup.

---

## 🗄️ Data Persistence

### JSON Indices (Atomic Writes)

Both `ExpiryStore` and `PrivateIndexStore` use:

```rust
// Write pattern: tmp file + atomic rename
let tmp_path = index_path.with_extension("json.tmp");
tokio::fs::write(&tmp_path, content).await?;
tokio::fs::rename(&tmp_path, &index_path).await?;
```

**Why**: Prevents corruption if process crashes mid-write.

### CouchDB Distributed Storage (`src/couchdb.rs`)

When `AGENFACT_COUCHDB_URL` is configured, `CouchDbStore` is initialized and attached to managed state:
- Uploads and metadata are synced into CouchDB Documents (`CouchDoc`).
- Background sweeper thread runs every 60 seconds querying the `_design/expiry/_view/by_expire_at` view to clean expired documents.

---

## 🧪 Development Commands

### Backend (Rust)

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run unit tests (93 tests)
cargo test

# Check without building
cargo check
```

### Frontend (Svelte + Vite)

```bash
cd web

# Install dependencies
pnpm install

# Dev server with hot reload
pnpm dev

# Production build → dist/
pnpm run build

# Unit test & type check
pnpm test
pnpm run check
```

### Docker Compose (App + CouchDB)

```bash
# Start full stack (App + CouchDB)
docker compose up --build
```

---

## 🔄 Quick Health Check

```bash
# Backend compiles & passes 93 tests?
cargo test

# Frontend type-checks and passes unit tests?
cd web && pnpm test && pnpm run check && pnpm run build

# Full stack runs locally with Docker Compose?
docker compose up --build
# Then open http://localhost:8080
```

---

_Last updated: 2026-07-31. For questions or clarifications, refer to the source code comments and inline documentation._
