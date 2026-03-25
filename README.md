# folio

A lightweight file storage server with a web interface, local expiry sweeper, and optional private-file protection via Cloudflare Access JWT.

- [Features](#features)
- [Usage](#usage)
- [Configuration](#configuration)
- [API](#api)
  - [`POST /uploads`](#post-uploads)
  - [`GET /files/:path`](#get-filespath)
  - [`GET /private-files/:path`](#get-private-filespath)
  - [`POST /files/:path`](#post-filespath)
  - [`PUT /files/:path`](#put-filespath)
  - [`DELETE /files/:path`](#delete-filespath)

## Features

- **Random filename generation**: `/uploads` generates unique 8-character filenames.
- **Custom file paths**: `/files/:path` supports explicit create/update/delete.
- **Path normalization**: file paths are normalized to reduce traversal risk.
- **Local expiry index + sweeper**: expiration is tracked in `data/expiry-index.json` and cleaned by an in-process background sweeper.
- **Private file redirect flow**: private-index (tracked in `data/private-files.json`) matches on `/files/:path` redirect to `/private-files/:path`.
- **Cloudflare Access verification**: `/private-files/:path` verifies `Cf-Access-Jwt-Assertion` or `bearer_token` JWT (cached for 1 hour).
- **Web interface**: built-in React/Vite upload UI.

## Usage

### Prerequisites

- [Rust and Cargo](https://rustup.rs/)

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

| Key | Environment Variable | Default | Description |
| --- | --- | --- | --- |
| `web_path` | `FOLIO_WEB_PATH` | `./web/dist` | Path to static web assets |
| `uploads_path` | `FOLIO_UPLOADS_PATH` | `./uploads` | Upload storage path |
| `data_path` | `FOLIO_DATA_PATH` | `./data` | Persistent metadata (index/state) path |

### Private access (Cloudflare Access)

| Environment Variable | Default | Description |
| --- | --- | --- |
| `FOLIO_CF_ACCESS_ISSUER` | `https://example.cloudflareaccess.com` | Expected JWT issuer |
| `FOLIO_CF_ACCESS_AUD` | _(empty)_ | Expected audience (required for production) |
| `FOLIO_CF_ACCESS_JWKS_URL` | `${ISSUER}/cdn-cgi/access/certs` | JWK Set URL for signature verification |
| `FOLIO_CF_ACCESS_HS256_SECRET` | _(unset)_ | Optional HS256 verifier secret (for local testing) |

Authorization is now per-file based. Access lists are defined during upload via the `authorized_emails` field.

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
#   "aud": "folio-app",
#   "sub": "user-123",
#   "email": "tester@example.com",
#   "exp": <future_timestamp>
# }

curl -H "Cf-Access-Jwt-Assertion: <your-hs256-token>" \
  "http://localhost:8000/private-files/<generated-id>.txt" -i
```

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

| Name | Required | Type | Description | Default |
| --- | :---: | --- | --- | --- |
| `expire` | ❌ | Query string | TTL (`10s`, `5m`, `24h`, `7d`) | `168h` |

- Form-data fields:

| Name | Required | Type | Description |
| --- | :---: | --- | --- |
| `file` | ✅ | File | File payload |
| `authorized_emails` | ❌ | String | Comma-separated list of emails allowed to access this file. Presence of this field automatically marks the file as private. |

Response:

- `201 Created`
- `Location` header: `/files/<generated-name>`

Example (Public):

```bash
curl -X POST -F "file=@sample.txt" "http://localhost:8000/uploads?expire=1h" -i
```

Example (Private):

```bash
curl -X POST \
  -F "file=@secret.txt" \
  -F "authorized_emails=bob@example.com,alice@example.com" \
  "http://localhost:8000/uploads" -i
```

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

- Requires request header: `Cf-Access-Jwt-Assertion` or `bearer_token`
- Validates JWT signature/issuer/audience/expiry
- Applies optional email/group allowlists

Response:

- `200 OK` when authorized
- `401 Unauthorized` on missing/invalid token (signature/issuer/audience/expiry)
- `403 Forbidden` on valid token but denied by email/group allowlist policy

Example:

```bash
curl -H "Cf-Access-Jwt-Assertion: <jwt>" http://localhost:8000/private-files/secret.txt
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

- Wiki Home: <https://forgejo.home-infra.weii.cloud/home-infra/folio.wiki/wiki/Home>
- Wiki Security Model: <https://forgejo.home-infra.weii.cloud/home-infra/folio.wiki/wiki/Security-Model>
- Wiki Roadmap: <https://forgejo.home-infra.weii.cloud/home-infra/folio.wiki/wiki/Roadmap>
