# AGENTS.md — Folio Coding Agent Guide

> **Purpose**: This document is optimized for AI coding agents (Claude Code, Codex, etc.) to quickly understand the codebase structure, conventions, and common modification patterns.

---

## 🎯 Quick Reference

| Aspect | Value |
|--------|-------|
| **Type** | Self-hosted file storage + sharing service |
| **Backend** | Rust 2024 edition, Rocket 0.5.1 |
| **Frontend** | React 19, Vite, TypeScript, Tailwind CSS 4 |
| **Auth** | Cloudflare Access JWT (RS256/JWKS or HS256) |
| **Storage** | Local filesystem + JSON indices |
| **Config** | Figment (TOML + env vars with `FOLIO_` prefix) |
| **CI/CD** | Gitea Actions (`.gitea/workflows/`) |

---

## 📁 Project Structure

```
folio/
├── src/                          # Rust backend
│   ├── main.rs                   # Entry point, route mounting, managed state
│   ├── config.rs                 # Figment config (TOML + env), path normalization
│   ├── auth.rs                   # JWT validation (RS256/JWKS + HS256), VerifiedIdentity guard
│   ├── files.rs                  # File CRUD (GET/POST/PUT/DELETE), ValidatedPath
│   ├── uploads.rs                # Random 8-char filename, multipart upload, TTL scheduling
│   ├── expiry.rs                 # Background sweeper (60s interval), ExpiryStore
│   ├── private_index.rs          # Private file authorization, PrivateIndexStore
│   └── test_utils.rs             # Test helpers (#[cfg(test)])
├── web/                          # React frontend
│   ├── src/
│   │   ├── App.tsx               # Main upload UI
│   │   ├── components/
│   │   │   ├── FileUploadZone.tsx # Drag & drop upload
│   │   │   ├── DownloadLink.tsx   # Short URL display + copy
│   │   │   └── ui/               # shadcn/ui primitives (Button, Card, Input, Label)
│   │   └── lib/utils.ts          # cn() helper (clsx + tailwind-merge)
│   ├── package.json              # bun install, bun dev, bun run dist
│   └── vite.config.ts
├── data/                         # Runtime data (created at runtime)
│   ├── expiry-index.json         # File expiration tracking
│   └── private-files.json        # Private file authorization lists
├── uploads/                      # Uploaded files (created at runtime)
├── .gitea/workflows/             # CI/CD pipelines
│   ├── rust.yml                  # Build + test + Trivy scan
│   └── docker.yml                # Docker build + Trivy + push
├── Dockerfile                    # Multi-stage: oven/bun (web) + rust (backend) → debian
└── Cargo.toml                    # Dependencies: rocket, figment, jsonwebtoken, reqwest
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
| `GET` | `/` | `FileServer` | Serve React SPA static assets |

---

## 🔐 Security Model

### Path Normalization (Defense in Depth)

1. **`ValidatedPath`** (`files.rs`): Rejects `..` in URL segments at Rocket parsing level
2. **`config::Folio::normalize_and_join()`** (`config.rs`): Strips `..`, `.`, root components; only `Normal` components kept

**When modifying**: Always use `build_full_upload_path()` or `build_full_data_path()` — never join paths manually.

### JWT Authentication

- **Request Guard**: `VerifiedIdentity` (`auth.rs:205`) implements `FromRequest`
- **Token Sources**: `Cf-Access-Jwt-Assertion` header (priority) OR `Authorization: Bearer ***
- **Verify Modes**:
  - **RS256 + JWKS**: Production. Fetches from Cloudflare, caches 1hr in `Mutex<Option<(JwkSet, Instant)>>`
  - **HS256**: Testing. Uses `FOLIO_CF_ACCESS_HS256_SECRET` env var

**When modifying auth**: Update both `verify_claims()` branches (RS256 + HS256).

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
| `web_path` | `FOLIO_WEB_PATH` | `./web/dist` | React build output path |
| `uploads_path` | `FOLIO_UPLOADS_PATH` | `./uploads` | Uploaded files storage |
| `data_path` | `FOLIO_DATA_PATH` | `./data` | JSON index files location |

### Cloudflare Access Settings (`auth.rs:from_env()`)

| Env Var | Default | Description |
|---------|---------|-------------|
| `FOLIO_CF_ACCESS_ISSUER` | `https://example.cloudflareaccess.com` | JWT issuer |
| `FOLIO_CF_ACCESS_AUD` | _(empty)_ | JWT audience (required for prod) |
| `FOLIO_CF_ACCESS_JWKS_URL` | `${ISSUER}/cdn-cgi/access/certs` | JWKS endpoint |
| `FOLIO_CF_ACCESS_HS256_SECRET` | _(unset)_ | HS256 secret (dev/test only) |

### Adding New Config Fields

1. Add field to `Folio` struct in `config.rs`
2. Add default value in `impl Default for Folio`
3. Reference via `&State<config::Folio>` in handlers or `config.build_full_*_path()` methods

---

## 🗄️ Data Persistence

### JSON Indices (Atomic Writes)

Both `ExpiryStore` and `PrivateIndexStore` use:

```rust
// Write pattern: tmp file + atomic rename
let tmp_path = index_path.with_extension("json.tmp");
std::fs::write(&tmp_path, content)?;
std::fs::rename(&tmp_path, &index_path)?;
```

**Why**: Prevents corruption if process crashes mid-write.

### `data/expiry-index.json`

```json
{
  "entries": [
    { "path": "/absolute/path/to/file.jpg", "expire_at_unix": 1704067200 }
  ]
}
```

- **Written by**: `ExpiryStore::schedule()` (called from `uploads::upload_file`)
- **Read/Cleaned by**: `ExpiryStore::sweep_once()` (background thread, 60s interval)

### `data/private-files.json`

```json
{
  "entries": [
    {
      "path": "relative/path/secret.pdf",
      "authorized_emails": ["bob@example.com", "alice@example.com"]
    }
  ]
}
```

- **Written by**: `PrivateIndexStore::mark_private()` (called from `uploads::upload_file` when `authorized_emails` form field present)
- **Read by**: `PrivateIndexStore::is_private()`, `get_entry()`

---

## 🧪 Development Commands

### Backend (Rust)

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Check without building
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

### Frontend (React + Vite)

```bash
cd web

# Install dependencies
bun install

# Dev server with hot reload
bun dev

# Production build → dist/
bun run dist

# Type check
bun run type-check  # or tsc --noEmit

# Lint
bun run lint
```

### Docker

```bash
# Build locally
docker build -t folio:local .

# Run
docker run -p 8080:8080 \
  -e FOLIO_CF_ACCESS_ISSUER=https://... \
  -e FOLIO_CF_ACCESS_AUD=... \
  folio:local
```

---

## 🛠️ Common Modification Patterns

### Adding a New Route

1. **Create handler** in appropriate module (`files.rs`, `uploads.rs`, or new module)
2. **Mount route** in `main.rs:rocket()`:
   ```rust
   .mount("/new-prefix", routes![new_handler])
   ```
3. **Add managed state** if needed:
   ```rust
   .manage(new_state)
   ```
4. **Update Cloudflare WAF rules** if the route needs write protection

### Adding a New Config Field

1. Add to `Folio` struct in `config.rs`:
   ```rust
   pub struct Folio {
       // ... existing fields
       pub new_field: String,
   }
   ```
2. Add default in `impl Default for Folio`
3. Access in handlers via `config: &State<config::Folio>`
4. Add env var documentation to `README.md`

### Modifying Auth Logic

1. **JWT Claims**: Update `AccessClaims` struct in `auth.rs`
2. **Verify Logic**: Modify `verify_claims()` — **both** RS256 and HS256 branches
3. **Request Guard**: Update `VerifiedIdentity::from_request()` if header extraction changes
4. **Update tests** in `auth.rs:mod tests`

### Adding Frontend Component

1. Create in `web/src/components/`
2. Use Tailwind CSS 4 for styling (no separate CSS files)
3. Use `lucide-react` for icons
4. Import in `App.tsx` or parent component
5. Run `bun run type-check` to verify

### Modifying Upload Flow

1. **`uploads::upload_file()`** handles:
   - Random filename generation (`UploadId::new(8)`)
   - Extension detection (Content-Type > filename)
   - File persistence (`form.file.copy_to()`)
   - Private marking (`private_store.mark_private()`)
   - Expiry scheduling (`expiry_store.schedule()`)
2. **Form fields**: Update `UploadForm` struct
3. **Query params**: Add to function signature (Rocket extracts automatically)

---

## 🐛 Key Pitfalls & Gotchas

### Path Handling

- ❌ **Never** use `PathBuf::join()` directly with user input
- ✅ **Always** use `config.build_full_upload_path()` or `build_full_data_path()`
- ✅ **Always** wrap user-provided paths in `ValidatedPath` for route params

### JWT Testing

- For local testing, set `FOLIO_CF_ACCESS_HS256_SECRET` (uses HS256)
- For production, **do not** set HS256 secret (forces RS256/JWKS)
- `aud` claim can be string OR array — both supported (`deserialize_aud` in `auth.rs`)

### Background Sweeper

- Runs in **separate thread** (not async task) — uses `std::thread::spawn()`
- Interval: **60 seconds** (hardcoded in `main.rs:rocket()`)
- Uses `Mutex<()>` to prevent race conditions on index file

### File Extension Detection

Priority order in `uploads.rs:upload_file()`:
1. Filename extension (if present)
2. Content-Type extension (if not `bin`)
3. No extension (fallback)

### Index File Locations

- **Expiry index**: `data/expiry-index.json` (paths are **absolute**)
- **Private index**: `data/private-files.json` (paths are **relative** to uploads root)

---

## 📚 Related Documentation

- **API Documentation**: See `README.md` for full API reference
- **Security Model**: `https://gitea.home-infra.weii.cloud/home-infra/folio/wiki/Security-Model`
- **CI/CD**: `.gitea/workflows/` — Rust test + Docker build on push/PR to `main`

---

## 🔄 Quick Health Check

```bash
# Backend compiles?
cargo check

# Tests pass?
cargo test

# Frontend builds?
cd web && bun run dist

# Full stack runs locally?
RUST_LOG=info cargo run
# Then open http://localhost:8000
```

---

_Last updated: 2026-06-21. For questions or clarifications, refer to the source code comments and inline documentation._
