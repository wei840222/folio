# folio

Simple HTTP server to save files.

- [Features](#features)
- [Usage](#usage)
- [File Storage](#file-storage)
- [API](#api)
  - [`POST /upload`](#post-upload)
  - [`POST /files/:path`](#post-filespath)
  - [`HEAD /files/:path`](#head-filespath)
  - [`GET /files/:path`](#get-filespath)
  - [`PUT /files/:path`](#put-filespath)
  - [`DELETE /files/:path`](#delete-filespath)

## Features

- **Simple file upload and download**: Upload files via POST/PUT and download via GET
- **Random filename generation**: `/upload` endpoint generates unique filenames automatically

## Usage

```
  -h, --help                                      help for folio
      --http-host string                          HTTP server host (default "0.0.0.0")
      --http-port int                             HTTP server port (default 8080)
      --http-max-upload-size int                  Maximum upload size in bytes (default 5242880)
      --file-root string                          Path to save uploaded files. (default "./uploads")
      --file-web-root string                      Path to the web root directory. This is used to serve the static files for the web interface. (default "./web/dist")
      --file-web-upload-path string               Path of the upload api response. (default "./files")
      --file-garbage-collection-pattern strings   Regular expressions to match files for garbage collection. Files matching these patterns will be deleted. (default [^\._.+,^\.DS_Store$])
```

The server supports configuration via command line flags, environment variables, and configuration files. Command line flags take precedence over environment variables, which take precedence over configuration files.

## File Storage

Files are stored in the filesystem at the location specified by `--file-root` flag (default: `./data/files`).
The `/upload` endpoint generates unique 8-character IDs for uploaded files, while `/files/:path` endpoints allow you to specify custom paths.

## API

### `POST /upload`

Uploads a new file with an automatically generated filename. The server generates a random 8-character ID and uses the original file extension.

#### Request

Content-Type
: `multipart/form-data`

Parameters:

| Name     | Required? | Type         | Description              | Default |
| -------- | :-------: | ------------ | ------------------------ | ------- |
| `file`   |     v     | Form Data    | A content of the file.   |         |
| `expire` |     x     | Query String | Expire time of the file. | 168h    |

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
curl -X POST -F file=@sample.txt http://localhost:8080/upload?expire=1h
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
curl -X POST -F file=@sample.txt "http://localhost:8080/files/test/sample.txt"
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
curl -I http://localhost:8080/files/foobar.txt
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
curl http://localhost:8080/files/sample.txt
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
curl -X PUT -F file=@sample.txt "http://localhost:8080/files/foobar.txt"
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

| StatusCode      | When                 |
| --------------- | -------------------- |
| `404 Not Found` | File path not found. |

#### Example

```bash
curl -X DELETE http://localhost:8080/files/foobar.txt
```

```
{"message":"file deleted successfully"}
```
