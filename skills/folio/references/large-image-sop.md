# Asset Workflow: Compression & Upload

This SOP describes how to handle large assets (images) when uploading to Folio to ensure reliability and performance.

## The Image Size Problem

- **Issue**: Uploading original PNG images exceeding **5MB** may trigger server errors (e.g., `400 Bad Request` or `502 Bad Gateway`).
- **Solution**: Compress and convert images to JPG format before uploading.

## Procedure

### 1. Compression and Conversion

Use the provided script to convert the image. Aim for a target size around **500KB**.

```bash
uv run --with Pillow python3 scripts/compress.py input.png output.jpg --quality 85
```

If compression fails, stop and report the error. Do not upload the original PNG as a fallback.

### 2. Upload and Retrieval

- **Security Warning**: Folio files are **PUBLIC** by default. Do not upload sensitive or private data unless using the `Private` (authorized_emails) feature.
- **Upload Command**: Use `scripts/upload.py`; do not substitute `curl` or construct an endpoint manually.
- **Location Header**: Do **NOT** guess the filename. Extract the temporary/random filename from the `location` header in the HTTP response.

```bash
uv run --with ua-generator --with requests scripts/upload.py \
  --file output.jpg \
  --expire 24h
```

The script must print the URL derived from the actual `Location` header. Report failure if the command exits non-zero or the response has no `Location`; never claim success or invent a URL.

If compression succeeds but upload returns `413`, stop and report the rejection. After obtaining a fresh confirmation and independently verifying that no file was created, a new quality-70 upload may be attempted. For a timeout, missing `Location`, `429`, `5xx`, or any ambiguous response, stop: the server may already have committed the first request, so a blind retry could create a duplicate public file. Report the failure without returning a guessed link.

```bash
# Only run this after independently confirming HTTP 413 and that no file was created.
uv run --with Pillow python3 scripts/compress.py input.png output-retry.jpg --quality 70 || exit 1
uv run --with ua-generator --with requests scripts/upload.py \
  --file output-retry.jpg \
  --expire 24h || exit 1
```

### 3. File Extension Control

The upload script derives the MIME type from the compressed file extension and sends it in the multipart form. Do not replace it with a hand-written request.
