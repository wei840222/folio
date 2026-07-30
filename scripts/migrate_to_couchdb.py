#!/usr/bin/env python3
"""
Agenfact -> CouchDB One-time Migration Script

Scans local uploads directories and JSON data indices (expiry-index.json, private-files.json),
creating corresponding CouchDB Documents with binary attachments.
"""

import argparse
import hashlib
import json
import mimetypes
import os
import sys
import urllib.parse
import urllib.request
import urllib.error


def parse_args():
    parser = argparse.ArgumentParser(
        description="Migrate local Agenfact file storage and JSON metadata into CouchDB."
    )
    parser.add_argument(
        "--uploads-dir",
        action="append",
        dest="uploads_dirs",
        help="Uploads directory path (can be specified multiple times). Default: ['./uploads']",
    )
    parser.add_argument(
        "--data-dir",
        default="./data",
        help="Data directory path containing expiry-index.json and private-files.json. Default: './data'",
    )
    parser.add_argument(
        "--couchdb-url",
        default=os.getenv("AGENFACT_COUCHDB_URL", "http://localhost:5984"),
        help="CouchDB base URL. Default: 'http://localhost:5984'",
    )
    parser.add_argument(
        "--couchdb-db",
        default=os.getenv("AGENFACT_COUCHDB_DB", "agenfact"),
        help="CouchDB target database name. Default: 'agenfact'",
    )
    parser.add_argument(
        "--couchdb-user",
        default=os.getenv("AGENFACT_COUCHDB_USER", ""),
        help="CouchDB basic auth username.",
    )
    parser.add_argument(
        "--couchdb-password",
        default=os.getenv("AGENFACT_COUCHDB_PASSWORD", ""),
        help="CouchDB basic auth password.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Scan files and metadata without writing to CouchDB.",
    )
    args = parser.parse_args()
    if not args.uploads_dirs:
        args.uploads_dirs = ["./uploads"]
    return args


def make_request(url, method="GET", headers=None, data=None, auth=None):
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("User-Agent", "Folio-Migrator/1.0")
    if headers:
        for k, v in headers.items():
            req.add_header(k, v)
    if auth:
        import base64
        creds = f"{auth[0]}:{auth[1]}".encode("utf-8")
        req.add_header("Authorization", f"Basic {base64.b64encode(creds).decode('ascii')}")
    try:
        with urllib.request.urlopen(req) as resp:
            content = resp.read()
            return resp.status, resp.headers, content
    except urllib.error.HTTPError as e:
        content = e.read()
        return e.code, e.headers, content


def load_json_indices(data_dir):
    expiry_map = {}  # relative_path -> expire_at_unix
    private_map = {} # relative_path -> { authorized_emails, owner }

    expiry_file = os.path.join(data_dir, "expiry-index.json")
    if os.path.isfile(expiry_file):
        try:
            with open(expiry_file, "r", encoding="utf-8") as f:
                data = json.load(f)
                for entry in data.get("entries", []):
                    p = entry.get("path", "")
                    exp = entry.get("expire_at_unix", 0)
                    if p and exp:
                        expiry_map[p] = exp
        except Exception as e:
            print(f"[WARN] Failed to parse {expiry_file}: {e}")

    private_file = os.path.join(data_dir, "private-files.json")
    if os.path.isfile(private_file):
        try:
            with open(private_file, "r", encoding="utf-8") as f:
                data = json.load(f)
                for entry in data.get("entries", []):
                    p = entry.get("path", "")
                    emails = entry.get("authorized_emails", [])
                    owner = entry.get("owner")
                    if p:
                        private_map[p] = {
                            "authorized_emails": emails,
                            "owner": owner
                        }
        except Exception as e:
            print(f"[WARN] Failed to parse {private_file}: {e}")

    return expiry_map, private_map


def ensure_couchdb_setup(url, db, auth, dry_run=False):
    if dry_run:
        print(f"[DRY-RUN] Would ensure database '{db}' and design documents exist on CouchDB at {url}")
        return

    db_url = f"{url.rstrip('/')}/{urllib.parse.quote(db, safe='')}"
    status, _, body = make_request(db_url, method="GET", auth=auth)
    if status == 404:
        print(f"[*] Database '{db}' does not exist. Creating...")
        status, _, body = make_request(db_url, method="PUT", auth=auth)
        if status not in (201, 200):
            print(f"[ERROR] Failed to create database '{db}': HTTP {status} - {body.decode('utf-8', 'ignore')}")
            sys.exit(1)
        print(f"[+] Database '{db}' created.")
    elif status != 200:
        print(f"[ERROR] Failed to check database '{db}': HTTP {status} - {body.decode('utf-8', 'ignore')}")
        sys.exit(1)

    # Ensure Expiry View (_design/expiry)
    view_url = f"{db_url}/_design/expiry"
    status, _, _ = make_request(view_url, method="GET", auth=auth)
    if status == 404:
        print("[*] Creating Expiry MapReduce View (_design/expiry)...")
        view_doc = {
            "_id": "_design/expiry",
            "views": {
                "by_expire_at": {
                    "map": "function (doc) { if (doc.expire_at_unix && doc.expire_at_unix > 0) { emit(doc.expire_at_unix, null); } }"
                }
            }
        }
        data = json.dumps(view_doc).encode("utf-8")
        status, _, body = make_request(view_url, method="PUT", headers={"Content-Type": "application/json"}, data=data, auth=auth)
        if status in (201, 200):
            print("[+] Expiry MapReduce View created.")
        else:
            print(f"[WARN] Could not create Expiry View: HTTP {status} - {body.decode('utf-8', 'ignore')}")


def normalize_rel_path(abs_or_rel_path, uploads_root):
    norm_path = os.path.normpath(abs_or_rel_path)
    norm_root = os.path.normpath(uploads_root)

    # If it's an absolute path or starts with uploads_root
    if os.path.isabs(norm_path) and norm_path.startswith(norm_root):
        rel = os.path.relpath(norm_path, norm_root)
    else:
        rel = norm_path
        # Strip leading ./ or uploads/
        if rel.startswith("./"):
            rel = rel[2:]
        if rel.startswith("uploads/"):
            rel = rel[len("uploads/"):]

    rel = rel.replace("\\", "/")
    return rel


def main():
    args = parse_args()
    auth = None
    if args.couchdb_user and args.couchdb_password:
        auth = (args.couchdb_user, args.couchdb_password)

    print("=== Folio -> CouchDB Migration Script ===")
    print(f"Uploads Directories : {args.uploads_dirs}")
    print(f"Data Directory       : {args.data_dir}")
    print(f"CouchDB Endpoint     : {args.couchdb_url} (DB: {args.couchdb_db})")
    print(f"Dry Run Mode         : {args.dry_run}")
    print("=========================================\n")

    expiry_map, private_map = load_json_indices(args.data_dir)
    print(f"[+] Loaded {len(expiry_map)} expiry entries and {len(private_map)} private entries from JSON.")

    ensure_couchdb_setup(args.couchdb_url, args.couchdb_db, auth, dry_run=args.dry_run)

    db_url = f"{args.couchdb_url.rstrip('/')}/{urllib.parse.quote(args.couchdb_db, safe='')}"

    migrated_count = 0
    total_bytes = 0
    errors_count = 0

    for uploads_dir in args.uploads_dirs:
        if not os.path.isdir(uploads_dir):
            print(f"[WARN] Uploads directory '{uploads_dir}' does not exist. Skipping...")
            continue

        abs_uploads_root = os.path.abspath(uploads_dir)
        print(f"\n[*] Scanning uploads directory: {abs_uploads_root} ...")

        for root, dirs, files in os.walk(abs_uploads_root):
            # Skip staging directory
            if ".folio-staging" in root:
                continue

            for fname in files:
                full_path = os.path.join(root, fname)
                rel_path = os.path.relpath(full_path, abs_uploads_root).replace("\\", "/")

                # Check metadata matching
                expire_at = None
                for k, v in expiry_map.items():
                    if normalize_rel_path(k, abs_uploads_root) == rel_path or k.endswith("/" + rel_path) or k == rel_path:
                        expire_at = v
                        break

                priv_info = None
                for k, v in private_map.items():
                    if normalize_rel_path(k, abs_uploads_root) == rel_path or k.endswith("/" + rel_path) or k == rel_path:
                        priv_info = v
                        break

                file_size = os.path.getsize(full_path)
                mime_type, _ = mimetypes.guess_type(full_path)
                if not mime_type:
                    mime_type = "application/octet-stream"

                doc_id = rel_path
                is_private = bool(priv_info and priv_info.get("authorized_emails"))
                authorized_emails = priv_info.get("authorized_emails", []) if priv_info else []
                owner = priv_info.get("owner") if priv_info else None

                doc_metadata = {
                    "_id": doc_id,
                    "path": rel_path,
                    "size_bytes": file_size,
                    "content_type": mime_type,
                    "is_private": is_private,
                    "authorized_emails": authorized_emails,
                }
                if expire_at:
                    doc_metadata["expire_at_unix"] = expire_at
                if owner:
                    doc_metadata["owner"] = owner

                if args.dry_run:
                    print(f"  [DRY-RUN] File: '{rel_path}' ({file_size} bytes) | Expiry: {expire_at} | Private: {is_private}")
                    migrated_count += 1
                    total_bytes += file_size
                    continue

                # Stage 1: Put Document Metadata
                doc_url = f"{db_url}/{urllib.parse.quote(doc_id, safe='')}"

                # Check if document already exists to get _rev
                status, _, resp_body = make_request(doc_url, method="GET", auth=auth)
                if status == 200:
                    try:
                        existing_doc = json.loads(resp_body.decode("utf-8"))
                        doc_metadata["_rev"] = existing_doc["_rev"]
                    except Exception:
                        pass

                payload = json.dumps(doc_metadata).encode("utf-8")
                status, _, resp_body = make_request(
                    doc_url,
                    method="PUT",
                    headers={"Content-Type": "application/json"},
                    data=payload,
                    auth=auth
                )

                if status not in (200, 201):
                    print(f"  [ERROR] Failed to PUT metadata for '{doc_id}': HTTP {status} - {resp_body.decode('utf-8', 'ignore')}")
                    errors_count += 1
                    continue

                res_json = json.loads(resp_body.decode("utf-8"))
                rev = res_json.get("rev")

                # Stage 2: PUT Attachment Stream
                att_url = f"{doc_url}/file?rev={rev}"
                with open(full_path, "rb") as f:
                    file_data = f.read()

                status, _, resp_body = make_request(
                    att_url,
                    method="PUT",
                    headers={"Content-Type": mime_type},
                    data=file_data,
                    auth=auth
                )

                if status in (200, 201):
                    print(f"  [+] Successfully migrated '{rel_path}' ({file_size} bytes) to CouchDB.")
                    migrated_count += 1
                    total_bytes += file_size
                else:
                    print(f"  [ERROR] Failed to PUT attachment for '{doc_id}': HTTP {status} - {resp_body.decode('utf-8', 'ignore')}")
                    errors_count += 1

    print("\n=========================================")
    print(f"Migration finished. Total files: {migrated_count}, Total size: {total_bytes} bytes, Errors: {errors_count}")
    print("=========================================")


if __name__ == "__main__":
    main()
