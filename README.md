# folio

A lightweight file storage server with a web interface and optional automatic expiration via Temporal.

- [Features](#features)
- [Usage](#usage)
- [Configuration](#configuration)
- [API](#api)
  - [`POST /uploads`](#post-uploads)
  - [`GET /files/:path`](#get-filespath)
  - [`POST /files/:path`](#post-filespath)
  - [`PUT /files/:path`](#put-filespath)
  - [`DELETE /files/:path`](#delete-filespath)

## Features

- **Random filename generation**: `/uploads` generates unique 8-character filenames.
- **Custom file paths**: `/files/:path` supports explicit create/update/delete.
- **Path normalization**: file paths are normalized to reduce traversal risk.
- **Expiration (implemented)**: uploaded files can be scheduled for deletion via Temporal.
- **Web interface**: built-in React/Vite upload UI.

## Usage

### Prerequisites

- [Rust and Cargo](https://rustup.rs/)
- A reachable Temporal server if expiration is required

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

| Key | Environment Variable | Default | Description |
| --- | --- | --- | --- |
| `web_path` | `FOLIO_WEB_PATH` | `./web/dist` | Path to static web assets |
| `uploads_path` | `FOLIO_UPLOADS_PATH` | `./uploads` | Upload storage path |
| `address` (Temporal) | `TEMPORAL_ADDRESS` | `localhost:7233` | Temporal server address |
| `namespace` (Temporal) | `TEMPORAL_NAMESPACE` | `default` | Temporal namespace |
| `task_queue` (Temporal) | `TEMPORAL_TASK_QUEUE` | `FOLIO:FILES` | Temporal task queue |

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
- `404 Not Found` if missing

Example:

```bash
curl http://localhost:8000/files/sample.txt
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

- Current main branch has **no token-based access control**.
- `garbage_collection_pattern` exists in config but GC cleanup is not implemented.
- If Temporal is unavailable, uploads still work but expiration scheduling is disabled.
