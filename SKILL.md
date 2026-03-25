---
name: folio
description: Upload, manage, and share files via Folio (https://folio.weii.cloud). Supports automatic expiration (TTL) and private files with email-based access control. Use when the user asks to upload, share, store, or create links for files.
---

# folio

Folio provides a lightweight file storage service with automated expiration and identity-aware access control via Cloudflare Access.

## Agent Instructions

When handling file requests:

1.  **Selection**: Confirm visibility (public/private) and desired TTL (e.g., `1h`, `24h`, `7d`). Default: `7d`.
2.  **Execution**: Use `run_command` with `curl` for all upload operations.
3.  **Reporting**: Return the uploaded file URL using the template provided below.

## Uploading Public Files

Use for non-sensitive data available to anyone with the link.

```bash
# Default (7d expiry)
curl -i -X POST -F 'file=@<path>;type=<mime>' https://folio.weii.cloud/uploads

# Custom expiry
curl -i -X POST -F 'file=@<path>;type=<mime>' "https://folio.weii.cloud/uploads?expire=<duration>"
```

## Uploading Private Files

Use for sensitive data. Requires a comma-separated list of authorized emails.

```bash
curl -i -X POST \
  -F 'file=@<path>;type=<mime>' \
  -F "authorized_emails=alice@example.com,bob@example.com" \
  https://folio.weii.cloud/uploads
```

## Example Interaction

**User**: "Share this report.pdf with dev-team@company.com for 24 hours."

**Agent Execution**:

```bash
curl -i -X POST \
  -F 'file=@report.pdf;type=application/pdf' \
  -F "authorized_emails=dev-team@company.com" \
  "https://folio.weii.cloud/uploads?expire=24h"
```

## Response Template

ALWAYS format the upload confirmation as follows:

```markdown
### ✅ File Uploaded Successfully

- **URL**: [https://folio.weii.cloud/files/<filename>](https://folio.weii.cloud/files/<filename>)
- **Visibility**: [Public | Private]
- **Expires**: <expiration_time>
- **Access Control**: <authorized_emails | None>
```

## Parameter Reference

- `file`: Multipart file field. Append `;type=<mime_type>` to ensure correct extension.
- `expire`: Duration string (e.g., `5m`, `1h`, `7d`).
- `authorized_emails`: CSV string of emails for private access.

## Important Notes

- **JWT Protection**: Private files are enforced by Cloudflare Access. Users will be prompted to authenticate when accessing the link.
- **Content-Type**: Missing `type` in `curl` may result in files having no extension.

## Installation

To add this skill to your AI assistant:

1.  Download the `SKILL.md` file from: [https://raw.githubusercontent.com/wei840222/folio/refs/heads/main/SKILL.md](https://raw.githubusercontent.com/wei840222/folio/refs/heads/main/SKILL.md)
2.  Place it in your agent's skills directory (e.g., `./skills/folio/SKILL.md`).
3.  Ensure the agent has access to `curl` in its environment.
