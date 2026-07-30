#!/usr/bin/env python3
# /// script
# dependencies = [
#   "requests",
#   "ua-generator",
# ]
# ///
import argparse
from datetime import datetime, timezone
import os
import re
import sys
from urllib.parse import urljoin, urlparse
import requests
import ua_generator


MAX_DURATION_VALUE = 10_000_000
DEFAULT_TIMEOUT_SECONDS = 120
DEFAULT_UPLOAD_URL = "https://folio.weii.cloud/uploads"
DURATION_PATTERN = re.compile(r"^(\d+)([smhd])$")
EMAIL_PATTERN = re.compile(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")


def duration_arg(value):
    match = DURATION_PATTERN.fullmatch(value)
    if not match or int(match.group(1)) > MAX_DURATION_VALUE:
        raise argparse.ArgumentTypeError(
            "expiration must be <number><s|m|h|d> with a value <= 10000000"
        )
    return value


def timeout_arg(value):
    try:
        timeout = float(value)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("timeout must be a positive number") from exc
    if timeout <= 0:
        raise argparse.ArgumentTypeError("timeout must be a positive number")
    return timeout


def upload_url_arg(value):
    parsed = urlparse(value)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise argparse.ArgumentTypeError("upload URL must be an absolute http(s) URL")
    return value


def emails_arg(value):
    emails = [email.strip() for email in value.split(",")]
    if not emails or any(not EMAIL_PATTERN.fullmatch(email) for email in emails):
        raise argparse.ArgumentTypeError("emails must be a comma-separated list of valid email addresses")
    return ",".join(emails)


def expiration_timestamp(expires_at):
    if isinstance(expires_at, bool) or not isinstance(expires_at, int) or expires_at < 0:
        raise ValueError("expires_at must be a non-negative integer")
    return datetime.fromtimestamp(expires_at, timezone.utc).isoformat()


def response_expiration(response):
    try:
        return expiration_timestamp(response.json().get("expires_at"))
    except (AttributeError, OSError, OverflowError, TypeError, ValueError):
        return None


def resolve_location(upload_url, location):
    if not location:
        raise ValueError("empty Location header")
    parsed = urlparse(location)
    if parsed.scheme in {"http", "https"} and parsed.netloc:
        resolved = location
    else:
        resolved = urljoin(upload_url, location)
    resolved_parts = urlparse(resolved)
    if resolved_parts.scheme not in {"http", "https"} or not resolved_parts.netloc:
        raise ValueError("Location header did not resolve to an absolute http(s) URL")
    return resolved


def resolve_preview_location(upload_url, response, location):
    try:
        preview_path = response.json().get("preview_url")
        if preview_path:
            return urljoin(upload_url, preview_path)
    except (AttributeError, OSError, TypeError, ValueError):
        pass
    if location:
        return urljoin(upload_url, f"/?preview={location}")
    return None


def main():
    parser = argparse.ArgumentParser(description="Folio Stealth Upload Script")
    parser.add_argument("--file", required=True, help="Path to the file to upload")
    parser.add_argument(
        "--expire",
        type=duration_arg,
        help="Expiration time (e.g., 1h, 24h, 7d)",
    )
    parser.add_argument(
        "--emails",
        type=emails_arg,
        help="Comma-separated authorized emails for private access",
    )
    parser.add_argument(
        "--url",
        type=upload_url_arg,
        default=DEFAULT_UPLOAD_URL,
        help=f"Folio upload URL (default: {DEFAULT_UPLOAD_URL})",
    )
    parser.add_argument(
        "--timeout",
        type=timeout_arg,
        default=DEFAULT_TIMEOUT_SECONDS,
        help="HTTP timeout in seconds (default: 120)",
    )

    args = parser.parse_args()

    # Generate random User-Agent
    ua = ua_generator.generate(device='desktop', browser='chrome')
    headers = {
        'User-Agent': ua.text
    }
    upload_token = os.environ.get('FOLIO_UPLOAD_TOKEN')
    if upload_token:
        headers['Authorization'] = f'Bearer {upload_token}'

    # Prepare parameters
    params = {}
    if args.expire:
        params['expire'] = args.expire

    data = {}
    if args.emails:
        data['authorized_emails'] = args.emails

    # Prepare files with proper filename and MIME type
    import mimetypes
    mime_type, _ = mimetypes.guess_type(args.file)
    if not mime_type:
        mime_type = 'application/octet-stream'

    filename = os.path.basename(args.file)

    try:
        with open(args.file, 'rb') as f:
            files = {'file': (filename, f, mime_type)}
            # Do not follow redirects: only the API's 201 response is a successful upload.
            response = requests.post(
                args.url,
                headers=headers,
                params=params,
                data=data,
                files=files,
                allow_redirects=False,
                timeout=args.timeout,
            )

        if response.status_code == 201:
            location = response.headers.get('location')
            if location:
                full_url = resolve_location(args.url, location)
                print(full_url)
                preview_url = resolve_preview_location(args.url, response, location)
                if preview_url:
                    print(f"Preview URL: {preview_url}")
                expires_at = response_expiration(response)
                print(f"Expires: {expires_at}" if expires_at else "Expires: unavailable")
            else:
                print(f"Error: Upload successful but no Location header found. Status: {response.status_code}")
                sys.exit(1)
        else:
            print(f"Error: Upload failed with status {response.status_code}")
            sys.exit(1)

    except FileNotFoundError:
        print(f"Error: File not found: {args.file}")
        sys.exit(1)
    except ValueError as e:
        print(f"Error: Invalid upload response: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"Error: An unexpected error occurred: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
