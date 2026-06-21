# folio

A lightweight file storage server with a web interface, local expiry sweeper, and optional private-file protection via Cloudflare Access JWT.

- [Features](#features)
- [Architecture](#architecture)
- [Usage](#usage)
- [Configuration](#configuration)
- [API](#api)
  - [`POST /uploads`](#post-uploads)
  - [`GET /files/:path`](#get-filespath)
  - [`GET /private-files/:path`](#get-private-filespath)
  - [`POST /files/:path`](#post-filespath)
  - [`PUT /files/:path`](#put-filespath)
  - [`DELETE /files/:path`](#delete-filespath)
- [Development](#development)
- [CI/CD](#cicd)

## Features

- **Random filename generation**: `/uploads` generates unique 8-character filenames.
- **Custom file paths**: `/files/:path` supports explicit create/update/delete.
- **Path normalization**: file paths are normalized to prevent directory traversal attacks.
- **Edge-level write protection**: Cloudflare WAF blocks anonymous POST/PUT/DELETE on `/files/*` to prevent abuse (see [Security Model](https://gitea.home-infra.weii.cloud/home-infra/folio/wiki/Security-Model)).
- **Local expiry index + sweeper**: expiration is tracked in `data/expiry-index.json` and cleaned by an in-process background sweeper.
- **Private file redirect flow**: private-index (tracked in `data/private-files.json`) matches on `/files/:path` redirect to `/private-files/:path`.
- **Cloudflare Access verification**: `/private-files/:path` verifies `Cf-Access-Jwt-Assertion` or standard `Authorization: Bearer *** JWT (RS256/JWKS with 1hr cache, or HS256 for local testing).
- **Web interface**: React 19 + Vite + TypeScript + Tailwind CSS 4 upload UI with drag & drop, short URL generation, and one-click copy.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Web Frontend (React 19)                     │
│           Vite + TypeScript + Tailwind CSS 4                 │
│         FileUploadZone → POST /uploads → DownloadLink        │
└────────────────────────────┬────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│                 Rust Backend (Rocket 0.5)                    │
├─────────────────────────────────────────────────────────────┤
│  /health          → health()               [health check]   │
│  /uploads  POST   → upload_file()          [random ID+TTL]  │
│  /files    GET    → get_file()             [public access]  │
│  /files    POST   → create_file()          [explicit path]  │
│  /files    PUT    → upsert_file()          [upsert]         │
│  /files    DELETE → delete_file()          [delete]         │
│  /private-files GET → get_private_file()   [JWT protected]  │
│  /                → FileServer             [SPA static]     │
└─────────────────────────────────────────────────────────────┘
        │                  │                  │
   ┌────▼────┐       ┌────▼─────┐      ┌────▼──────┐
   │ config  │       │ Expiry   │      │ Private   │
   │ (Figment│       │ Store    │      │ Index     │
   │  TOML+  │       │ (60s     │      │ Store     │
   │  ENV)   │       │ sweeper) │      │ (JSON)    │
   └─────────┘       └──────────┘      └───────────┘
                                          │
                                    ┌─────▼─────┐
                                    │  Access   │
                                    │  Auth     │
                                    │ (JWT/     │
                                    │  JWKS)    │
                                    └───────────┘
```

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust 2024 edition, Rocket 0.5, Figment (config), jsonwebtoken, reqwest |
| Frontend | React 19, Vite, TypeScript, Tailwind CSS 4, Radix UI, Lucide icons |
| Storage | Local filesystem (`uploads/`) + JSON indices (`data/`) |
| Auth | Cloudflare Access JWT (RS256/JWKS or HS256) |
| CI/CD | Gitea Actions (Rust test + Trivy scan + Docker build-push) |

## Usage

### Prerequisites

- [Rust and Cargo](https://rustup.rs/) (2024 edition)
- [Bun](https://bun.sh/) (for frontend development)

### Running the Server

**Linux/macOS:**

```bash
RUST_LOG=info cargo run
```

**Windows (PowerShell):**

```powershell
$env:RUST_LOG="info"; cargo run
```

With custom upload limits:

**Linux/macOS:**

```bash
RUST_LOG=info ROCKET_LIMITS='{file="5 MiB"}' cargo run
```

**Windows (PowerShell):**

```powershell
$env:RUST_LOG="info"; $env:ROCKET_LIMITS='{file="5 MiB"}'; cargo run
```

## Configuration

Configured with `Folio.toml` and/or environment variables.

### Core

| Key            | Environment Variable | Default      | Description                            |
| -------------- | -------------------- | ------------ | -------------------------------------- |
| `web_path`     | `FOLIO_WEB_PATH`     | `./web/dist` | Path to static web assets              |
| `uploads_path` | `FOLIO_UPLOADS_PATH` | `./uploads`  | Upload storage path                    |
| `data_path`    | `FOLIO_DATA_PATH`    | `./data`     | Persistent metadata (index/state) path |

### Private access (Cloudflare Access)

| Environment Variable           | Default                                | Description                                        |
| ------------------------------ | -------------------------------------- | -------------------------------------------------- |
| `FOLIO_CF_ACCESS_ISSUER`       | `https://example.cloudflareaccess.com` | Expected JWT issuer                                |
| `FOLIO_CF_ACCESS_AUD`          | _(empty)_                              | Expected audience (required for production)        |
| `FOLIO_CF_ACCESS_JWKS_URL`     | `${ISSUER}/cdn-cgi/access/certs`       | JWK Set URL for signature verification             |
| `FOLIO_CF_ACCESS_HS256_SECRET` | _(unset)_                              | Optional HS256 verifier secret (for local testing) |

Authorization is per-file based. Access lists are defined during upload via the `authorized_emails` field.

### Local Development / Testing (HS256)

When `FOLIO_CF_ACCESS_HS256_SECRET` is set, Folio will use this secret to verify JWTs instead of fetching JWKS from Cloudflare. This is useful for manual testing without a real Cloudflare Access setup.

**Example Configuration (.env):**

```bash
FOLIO_CF_ACCESS_ISSUER=https://issuer.example.com
FOLIO_CF_ACCESS_AUD=folio-app
FOLIO_CF_ACCESS_HS256_SECRET=my-local-secret
```

**Testing with curl:**

1.  **Upload a private file** for a specific user:

```bash
curl -X POST \
  -F "file=@secret.txt" \
  -F "authorized_emails=tester@example.com" \
  "http://localhost:8000/uploads" -i
```

2.  **Access the file** using a generated HS256 token (you can use [jwt.io](https://jwt.io) to generate one with `my-local-secret`):

```bash
# Token payload should include:
# {
#   "iss": "https://issuer.example.com",
#   "aud": "folio-app",              // Can also be an array: ["folio-app"]
#   "sub": "user-123",
#   "email": "tester@example.com",
#   "exp": <future_timestamp>
# }

curl -H "Cf-Access-Jwt-Assertion: *** \
  "http://localhost:8000/private-files/<generated-id>.txt" -i
```

**Note:** The `aud` (audience) field can be either a string or an array. Cloudflare Access typically sends it as an array `["audience-id"]`. Both formats are supported.

### Example `.env` (production baseline)

```bash
FOLIO_CF_ACCESS_ISSUER=https://<team>.cloudflareaccess.com
FOLIO_CF_ACCESS_AUD=<your-access-audience>
FOLIO_CF_ACCESS_JWKS_URL=https://<team>.cloudflareaccess.com/cdn-cgi/access/certs
```

## API

### `POST /uploads`

Upload a file with generated ID-based filename.

- Content-Type: `multipart/form-data`
- Query parameters:

| Name     | Required | Type         | Description                    | Default |
| -------- | :------: | ------------ | ------------------------------ | ------- |
| `expire` |    ❌    | Query string | TTL (`10s`, `5m`, `24h`, `7d`) | `168h`  |

- Form-data fields:

| Name                | Required | Type   | Description                                                                                                                 |
| ------------------- | :------: | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| `file`              |    ✅    | File   | File payload                                                                                                                |
| `authorized_emails` |    ❌    | String | Comma-separated list of emails allowed to access this file. Presence of this field automatically marks the file as private. |

**Note on file extensions:**

The server determines file extension in the following order:

1. **Content-Type from multipart field** (recommended) - explicitly specify using `curl -F` syntax
2. **Original filename extension** - fallback if Content-Type is missing or generic

Response:

- `201 Created`
- `Location` header: `/files/<generated-name>`

**Example (Public):**

```bash
# Recommended: explicitly set Content-Type to ensure correct extension
curl -X POST \
  --form 'file=@sample.txt;type=text/plain' \
  "http://localhost:8000/uploads?expire=1h" -i

# Alternative: using -F (shorter syntax, same result)
curl -X POST -F "file=@sample.txt;type=text/plain" \
  "http://localhost:8000/uploads?expire=1h" -i
```

**Example (Private):**

```bash
curl -X POST \
  --form 'file=@secret.pdf;type=application/pdf' \
  -F "authorized_emails=bob@example.com,alice@example.com" \
  "http://localhost:8000/uploads" -i
```

**Reference:** See [this article](https://ryanseddon.com/hacking/content-type-formdata-curl/) for detailed `curl` Content-Type syntax.

### `GET /files/:path`

Download file content from uploads path.

- `200 OK` on success
- `302 Found` to `/private-files/:path` if file is marked private
- `404 Not Found` if missing

Example:

```bash
curl -i http://localhost:8000/files/sample.txt
```

### `GET /private-files/:path`

Read private file content.

- Requires request header: `Cf-Access-Jwt-Assertion` or `Authorization: Bearer ***
- Validates JWT signature/issuer/audience/expiry
- The `aud` field can be either a string or an array (Cloudflare Access sends it as array)
- Checks per-file email authorization list

Response:

- `200 OK` when authorized
- `401 Unauthorized` on missing/invalid token (signature/issuer/audience/expiry)
- `403 Forbidden` on valid token but email not in file's authorized list

Example:

```bash
curl -H "Cf-Access-Jwt-Assertion: *** http://localhost:8000/private-files/secret.txt
```

### `POST /files/:path`

Create file at explicit path.

- `201 Created` on success
- `409 Conflict` if already exists

Example:

```bash
curl -X POST -F "file=@sample.txt" "http://localhost:8000/files/docs/sample.txt"
```

### `PUT /files/:path`

Create or overwrite file at explicit path.

- `201 Created` if new
- `200 OK` if overwritten

Example:

```bash
curl -X PUT -F "file=@sample.txt" "http://localhost:8000/files/docs/sample.txt"
```

### `DELETE /files/:path`

Delete file at explicit path.

- `200 OK` on success
- `404 Not Found` if missing
- `400 Bad Request` if path is a directory

Example:

```bash
curl -X DELETE "http://localhost:8000/files/docs/sample.txt"
```

## Development

### Backend

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test
```

### Frontend

```bash
cd web

# Install dependencies (requires Bun)
bun install

# Development server with hot reload
bun dev

# Production build
bun run dist
```

### Project Structure

```
folio/
├── src/                    # Rust backend source
│   ├── main.rs            # Application entry point, route mounting
│   ├── config.rs          # Figment-based configuration (TOML + env)
│   ├── auth.rs            # Cloudflare Access JWT validation (RS256/HS256)
│   ├── files.rs           # File CRUD operations, path validation
│   ├── uploads.rs         # Random filename generation, multipart uploads
│   ├── expiry.rs          # Background sweeper for file expiration
│   ├── private_index.rs   # Private file metadata (authorized emails)
│   └── test_utils.rs      # Test helpers
├── web/                   # React frontend
│   ├── src/
│   │   ├── App.tsx        # Main upload UI
│   │   ├── components/    # UI components (FileUploadZone, DownloadLink)
│   │   └── lib/           # Utilities
│   ├── package.json
│   └── vite.config.ts
├── data/                  # Runtime data (created at runtime)
│   ├── expiry-index.json  # File expiration tracking
│   └── private-files.json # Private file authorization
├── uploads/               # Uploaded files (created at runtime)
├── .gitea/workflows/      # CI/CD pipelines
│   ├── rust.yml           # Rust test + Trivy scan
│   └── docker.yml         # Docker build + push
└── Dockerfile             # Multi-stage build (Bun + Rust)
```

## CI/CD

The project uses Gitea Actions for CI/CD.

### Workflows

| Workflow | Trigger | Description |
|----------|---------|-------------|
| `rust.yml` | Push/PR to `main`, tags `*.*.*` | Build, test, Trivy vulnerability scan |
| `docker.yml` | Push/PR to `main`, tags `*.*.*` | Docker build, Trivy image scan, push to registry |

### Registry

Docker images are pushed to: `registry-gitea.home-infra.weii.cloud/home-infra/folio`

## Rollout checklist (dev → staging → production)

1. Configure environment variables (`FOLIO_CF_ACCESS_*`) and restart service.
2. Verify public flow:
   - `GET /files/<public-file>` returns `200`
3. Verify private redirect flow:
   - `GET /files/<private-file>` returns `302` with `Location: /private-files/<private-file>`
4. Verify auth failures:
   - no `Cf-Access-Jwt-Assertion` header on `/private-files/...` returns `401`
   - invalid token returns `401`
   - valid token but email not in the file's `authorized_emails` list returns `403`
5. Verify authorized access:
   - valid token + email matches the list returns `200`
6. Check logs for deny audit entries (code/status/path/method) and ensure no token leakage.

## Notes

- Local persistent data files:
  - `data/expiry-index.json`
  - `data/private-files.json`
- `garbage_collection_pattern` exists in config but GC cleanup is not implemented.

## Related docs

- Wiki Home: <https://gitea.home-infra.weii.cloud/home-infra/folio/wiki/Home>
- Wiki Security Model: <https://gitea.home-infra.weii.cloud/home-infra/folio/wiki/Security-Model>
- Wiki Roadmap: <https://gitea.home-infra.weii.cloud/home-infra/folio/wiki/Roadmap>
