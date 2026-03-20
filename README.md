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
- **Local expiry index + sweeper**: expiration is tracked in `uploads/.expiry-index.json` and cleaned by an in-process background sweeper.
- **Private file redirect flow**: private-index matches on `/files/:path` redirect to `/private-files/:path`.
- **Cloudflare Access verification**: `/private-files/:path` verifies `Cf-Access-Jwt-Assertion` JWT.
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

### Private access (Cloudflare Access)

| Environment Variable | Default | Description |
| --- | --- | --- |
| `FOLIO_CF_ACCESS_ISSUER` | `https://example.cloudflareaccess.com` | Expected JWT issuer |
| `FOLIO_CF_ACCESS_AUD` | _(empty)_ | Expected audience (required for production) |
| `FOLIO_CF_ACCESS_JWKS_URL` | `${ISSUER}/cdn-cgi/access/certs` | JWK Set URL for signature verification |
| `FOLIO_CF_ACCESS_ALLOWED_EMAILS` | _(empty)_ | Comma-separated allowed emails |
| `FOLIO_CF_ACCESS_ALLOWED_GROUPS` | _(empty)_ | Comma-separated allowed groups |

If allowlists are empty, JWT validity gates access and no extra allowlist filtering is applied.

## API

### `POST /uploads`

Upload a file with generated ID-based filename.

- Content-Type: `multipart/form-data`
- Query parameters:

| Name | Required | Type | Description | Default |
| --- | :---: | --- | --- | --- |
| `file` | ✅ | Form data | File payload | |
| `expire` | ❌ | Query string | TTL (`10s`, `5m`, `24h`, `7d`) | `168h` |

Response:

- `201 Created`
- `Location` header: `/files/<generated-name>`

Example:

```bash
curl -X POST -F "file=@sample.txt" "http://localhost:8000/uploads?expire=1h" -i
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

- Requires request header: `Cf-Access-Jwt-Assertion`
- Validates JWT signature/issuer/audience/expiry
- Applies optional email/group allowlists

Response:

- `200 OK` when authorized
- `401 Unauthorized` on missing/invalid token or unauthorized identity

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

## Notes

- Local metadata files:
  - `uploads/.expiry-index.json`
  - `uploads/.private-files.json`
- `garbage_collection_pattern` exists in config but GC cleanup is not implemented.
