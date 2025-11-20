# folio

A lightweight file storage server with automatic expiration and web interface.

- [Features](#features)
- [Usage](#usage)
- [File Storage](#file-storage)
- [API](#api)
  - [`POST /uploads`](#post-uploads)
  - [`POST /files/:path`](#post-filespath)
  - [`HEAD /files/:path`](#head-filespath)
  - [`GET /files/:path`](#get-filespath)
  - [`PUT /files/:path`](#put-filespath)
  - [`DELETE /files/:path`](#delete-filespath)

## Features

- **Random filename generation**: `/uploads` endpoint generates unique 8-character filenames automatically
- **Custom file paths**: Specify your own file paths using `/files/:path` endpoints
- **Directory Traversal Protection**: Secure path normalization prevents accessing files outside the uploads directory
- **File expiration**: Set expiration time for uploaded files (default: 168h/7 days) **(Not Implemented)**
- **File operations**: Support HEAD, GET, POST, PUT, and DELETE operations
- **Garbage collection**: Automatic cleanup of files matching specified patterns **(Not Implemented)**
- **Web interface**: Built-in web UI for file management

## Usage

### Prerequisites

- [Rust and Cargo](https://rustup.rs/)

### Running the Server

Start the server with default settings:

**Linux/macOS:**

```bash
RUST_LOG=info cargo run
```

**Windows (PowerShell):**

```powershell
$env:RUST_LOG="info"; cargo run
```

With custom limits:

**Linux/macOS:**

```bash
RUST_LOG=info ROCKET_LIMITS='{file="5 MiB"}' cargo run
```

**Windows (PowerShell):**

```powershell
$env:RUST_LOG="info"; $env:ROCKET_LIMITS='{file="5 MiB"}'; cargo run
```

## Configuration

The server can be configured via `Folio.toml` or environment variables.

| Key            | Environment Variable | Default      | Description                   |
| -------------- | -------------------- | ------------ | ----------------------------- |
| `web_path`     | `FOLIO_WEB_PATH`     | `./web/dist` | Path to the static web files. |
| `uploads_path` | `FOLIO_UPLOADS_PATH` | `./uploads`  | Path to store uploaded files. |

## File Storage

Files are stored in the filesystem at the location specified by `--file-uploads` flag (default: `./uploads`).
The `/uploads` endpoint generates unique 8-character IDs for uploaded files, while `/files/:path` endpoints allow you to specify custom paths.

## API

### `POST /uploads`

Uploads a new file with an automatically generated filename. The server generates a random 8-character ID and uses the original file extension.

#### Request

Content-Type
: `multipart/form-data`

Parameters:

| Name     | Required? | Type         | Description                                    | Default |
| -------- | :-------: | ------------ | ---------------------------------------------- | ------- |
| `file`   |     v     | Form Data    | A content of the file.                         |         |
| `expire` |     x     | Query String | Expire time of the file **(Not Implemented)**. | 168h    |

#### Response

##### On Successful

Status Code
: `201 Created`

Content-Type
: `application/json`

Body:

| Name      | Type     | Description                             |
| --------- | -------- | --------------------------------------- |
| `message` | `string` | Success message.                        |
| `path`    | `string` | A path to access this file in this API. |

##### On Failure

| StatusCode                     | When                                   |
| ------------------------------ | -------------------------------------- |
| `400 Bad Request`              | Invalid request or missing file field. |
| `413 Request Entity Too Large` | File size exceeds the upload limit.    |

#### Example

```bash
echo 'Hello, world!' > sample.txt
curl -X POST -F file=@sample.txt http://localhost:8000/uploads?expire=1h
```

```
{"message":"file created successfully","path":"abc12345.txt"}
```

### `POST /files/:path`

Uploads a file to a specific path. Creates a new file at the specified path.

#### Parameters

| Name    | Required? | Type      | Description            | Default |
| ------- | :-------: | --------- | ---------------------- | ------- |
| `:path` |     v     | `string`  | Path to the file.      |         |
| `file`  |     v     | Form Data | A content of the file. |         |

#### Response

##### On Successful

Status Code
: `201 Created`

Content-Type
: `application/json`

Body:

| Name      | Type     | Description      |
| --------- | -------- | ---------------- |
| `message` | `string` | Success message. |

##### On Failure

| StatusCode                     | When                                           |
| ------------------------------ | ---------------------------------------------- |
| `400 Bad Request`              | Invalid file path or missing file field.       |
| `409 Conflict`                 | There is already a file at the specified path. |
| `413 Request Entity Too Large` | File size exceeds the upload limit.            |

#### Example

```bash
curl -X POST -F file=@sample.txt "http://localhost:8000/files/test/sample.txt"
```

```
{"message":"file created successfully"}
```

### `HEAD /files/:path`

Check existence of a file.

#### Request

Parameters:

| Name    | Required? | Type     | Description         | Default |
| ------- | :-------: | -------- | ------------------- | ------- |
| `:path` |     v     | `string` | A path to the file. |         |

#### Response

##### On Successful

Status Code
: `200 OK`

Body
: Not Available

##### On Failure

| StatusCode      | When                                               |
| --------------- | -------------------------------------------------- |
| `404 Not Found` | No such file on the server or path is a directory. |

#### Example

```bash
curl -I http://localhost:8000/files/foobar.txt
```

### `GET /files/:path`

Downloads a file.

#### Request

Parameters:

| Name    | Required? | Type     | Description         | Default |
| ------- | :-------: | -------- | ------------------- | ------- |
| `:path` |     v     | `string` | A path to the file. |         |

#### Response

##### On Successful

Status Code
: `200 OK`

Content-Type
: Depends on the content.

Body
: The content of the requested file.

##### On Failure

Content-Type
: `application/json`

| StatusCode      | When                                          |
| --------------- | --------------------------------------------- |
| `404 Not Found` | There is no such file or path is a directory. |

#### Example

```bash
curl http://localhost:8000/files/sample.txt
```

```
Hello, world!
```

### `PUT /files/:path`

Uploads a file to a specific path. Allows overwriting existing files.

#### Parameters

| Name    | Required? | Type      | Description            | Default |
| ------- | :-------: | --------- | ---------------------- | ------- |
| `:path` |     v     | `string`  | Path to the file.      |         |
| `file`  |     v     | Form Data | A content of the file. |         |

#### Response

##### On Successful

Status Code
: `201 Created` (for new files) or `200 OK` (for overwritten files)

Content-Type
: `application/json`

Body:

| Name      | Type     | Description      |
| --------- | -------- | ---------------- |
| `message` | `string` | Success message. |

##### On Failure

| StatusCode                     | When                                     |
| ------------------------------ | ---------------------------------------- |
| `400 Bad Request`              | Invalid file path or missing file field. |
| `413 Request Entity Too Large` | File size exceeds the upload limit.      |

#### Example

```bash
curl -X PUT -F file=@sample.txt "http://localhost:8000/files/foobar.txt"
```

```
{"message":"file created successfully"}
```

### `DELETE /files/:path`

Delete a file to a specific path.

#### Parameters

| Name    | Required? | Type     | Description       | Default |
| ------- | :-------: | -------- | ----------------- | ------- |
| `:path` |     v     | `string` | Path to the file. |         |

#### Response

##### On Successful

Status Code
: `200 OK`

Content-Type
: `application/json`

Body:

| Name      | Type     | Description      |
| --------- | -------- | ---------------- |
| `message` | `string` | Success message. |

##### On Failure

| StatusCode        | When                 |
| ----------------- | -------------------- |
| `400 Bad Request` | Path is a directory. |
| `404 Not Found`   | File path not found. |

#### Example

```bash
curl -X DELETE http://localhost:8000/files/foobar.txt
```

```
{"message":"file deleted successfully"}
```
