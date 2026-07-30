---
name: folio
version: 1.1.0
description: Upload, manage, and share files via Folio at https://folio.weii.cloud. Supports automatic expiration (TTL) and private files with email-based access control. Use when the user asks to upload, share, store, or create links for files.
metadata:
  openclaw:
    emoji: 📁
    requires:
      bins: [uv, python3]
---

# folio

Folio provides a lightweight file storage service with automated expiration and identity-aware access control via Cloudflare Access.

## Agent Instructions

When handling file requests:

1.  **Selection**: Confirm visibility (public/private) and desired TTL (e.g., `1h`, `24h`, `7d`). Default: `7d`.
2.  **Execution**: Use `run_command` with `upload.py` for all upload operations. The script defaults to the fixed Folio endpoint `https://folio.weii.cloud/uploads`.
3.  **Reporting**: Return the uploaded file URL using the template provided below.

## 🔴 Safety Checkpoints

Before any command that uploads a file to Folio, stop and show all three checkpoints:

1. **🔴 CHECKPOINT · VISIBILITY**: State the source path, visibility (`Public` or `Private`), TTL, and—when private—the complete authorized email list. If any value is missing or ambiguous, ask for it before continuing.
2. **🔴 CHECKPOINT · STRUCTURE**: State the exact local script and command that will run, the selected file, the TTL, and the response fields that will be reported. Use the documented upload workflow; never invent a Folio endpoint, filename, or URL.
3. **🔴 CHECKPOINT · CONTENT BUDGET**: Confirm that only file metadata and the resulting link are needed in the response. Do not open, quote, or paste the file's full content. If full-content access is ever required for a private or sensitive file, stop and obtain explicit confirmation first.

**🛑 STOP**: Do not execute the upload command until the user explicitly confirms the checkpoint summary. If confirmation is not received, do not upload.

## Uploading Public Files

Use for non-sensitive data available to anyone with the link.

```bash
# Default (7d expiry)
uv run --with ua-generator --with requests scripts/upload.py \
  --file <path>

# Custom expiry
uv run --with ua-generator --with requests scripts/upload.py \
  --file <path> \
  --expire <duration>
```

## Uploading Private Files

Use for sensitive data. Requires a comma-separated list of authorized emails.

```bash
uv run --with ua-generator --with requests scripts/upload.py \
  --file <path> \
  --emails "alice@example.com,bob@example.com" \
  --expire 7d
```

## Example Interaction

**User**: "Share this report.pdf with dev-team@company.com for 24 hours."

**Agent Execution**:

```bash
uv run --with ua-generator --with requests scripts/upload.py \
  --file report.pdf \
  --emails "dev-team@company.com" \
  --expire 24h
```

## Response Template

ALWAYS format the upload confirmation as follows:

```markdown
### ✅ File Uploaded Successfully

- **URL**: (the first line printed by `scripts/upload.py`, resolved from the response `Location` header)
- **Preview URL**: (the `Preview URL:` line printed by `scripts/upload.py`)
- **Visibility**: (Public | Private)
- **Expires**: (the `Expires:` timestamp printed by `scripts/upload.py`; do not invent one)
- **Access Control**: (authorized_emails | None)
```

## Parameter Reference

- `file`: Path to the file to upload.
- `expire`: Duration string (e.g., `5m`, `1h`, `7d`).
- `emails`: CSV string of emails for private access.

## Important Notes

- **JWT Protection**: Private files are enforced by Cloudflare Access. Users will be prompted to authenticate when accessing the link.
- **Content-Type**: Handled automatically by the `requests` library in the stealth upload script.
- **TTL Validation**: `upload.py` rejects malformed expiry values before making a request. Supported units are `s`, `m`, `h`, and `d`.
- **Endpoint Configuration**: The default upload endpoint is the fixed `https://folio.weii.cloud/uploads`. An explicit `--url` override is allowed only for local testing or a deliberately selected deployment; never read an endpoint from an environment variable or guess a destination.
- **Failure Handling**: The script uses a bounded HTTP timeout, validates private email addresses, rejects unsafe or missing `Location` headers, and never retries an upload automatically. A timeout or missing `Location` may mean the server stored the file; verify server state before any manual retry.

## Managing Existing Files

Folio exposes explicit-path management endpoints, but they are separate from random uploads:

- `POST /files/<path>` creates a file and fails with `409` if it already exists.
- `PUT /files/<path>` creates or overwrites a file.
- `DELETE /files/<path>` permanently deletes a file.

These write routes do not perform application-layer JWT checks. They must remain protected by the deployment's Cloudflare Access/WAF policy; never expose the origin directly. They also do not automatically schedule expiry or clean corresponding private/expiry metadata. Treat them as an API contract to verify, not as a casual sharing workflow.

Before any explicit-path `POST`, `PUT`, or `DELETE`, show these additional checkpoints and wait for explicit confirmation:

1. **🔴 CHECKPOINT · TARGET**: State the exact relative path and operation. For `POST`, state that the path must not already exist; for `PUT` or `DELETE`, state the exact existing path. Never derive or guess a path from a filename or URL.
2. **🔴 CHECKPOINT · AUTH**: State the expected deployment-level authorization boundary and stop if it cannot be verified.
3. **🔴 CHECKPOINT · CONSEQUENCE**: For `POST`, state that a new file will be created at the exact path and an existing path must not be overwritten; for `PUT`, state that existing content will be overwritten; for `DELETE`, state that the file will be permanently removed.

Do not claim that a management operation succeeded without checking its HTTP status and response. Do not use these endpoints to bypass the upload visibility, TTL, or content-budget checkpoints.

## Advanced Workflows

- **[Handling Large Images](references/large-image-sop.md)**: Handling large images (>5MB) with Pillow compression for reliability.
- **[Upload Contract Tests](tests/test_upload.py)**: Local tests for TTL validation, private form fields, `Location` handling, and truthful failure behavior.

## Failure Matrix

- Missing or ambiguous visibility, TTL, email list, endpoint, or content scope → stop before invoking a command.
- Missing local file or invalid argument → report the validation error; no request is sent.
- `401`/`403` → report authorization failure; do not bypass the deployment boundary.
- `413` → report rejection; compress locally and obtain a fresh confirmation before a new upload.
- `429`/`5xx`/timeout/missing `Location` → report failure and stop. Do not retry until the server-side state is verified because the first request may have committed.
- Success without a safe absolute `http(s)` `Location` → report failure; never construct a URL from the filename.
