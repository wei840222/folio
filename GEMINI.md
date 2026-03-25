# Folio - Lightweight File Storage Service

Folio is a self-hosted file storage server designed for simplicity and security. It features a modern web interface, automatic file expiration, and private-file protection integrated with Cloudflare Access.

## Project Overview

- **Backend**: Built with **Rust** using the **Rocket** web framework.
- **Frontend**: A **React 19** application powered by **Vite**, **TypeScript**, and **Tailwind CSS 4**.
- **Security**: Supports **Cloudflare Access JWT** for private file authorization and path normalization to prevent directory traversal.
- **Persistence**: Files are stored locally in an `uploads/` directory; metadata for expiration and private access is kept in `data/`.

## Architecture

- `src/`: Rust backend source code.
    - `main.rs`: Application entry point and route mounting.
    - `config.rs`: Figment-based configuration management.
    - `expiry.rs`: Background worker for automatic file deletion after a TTL.
    - `auth.rs`: JWT validation logic for Cloudflare Access.
    - `files.rs`: Core file handling logic (GET/POST/PUT/DELETE).
    - `uploads.rs`: Logic for random filename generation and multipart uploads.
- `web/`: React frontend source code.
    - `src/components/`: UI components using Radix UI and Lucide icons.
    - `src/App.tsx`: Main dashboard for file uploads.
- `data/`: JSON indices for expiry tracking and private file metadata.
- `uploads/`: Physical storage for uploaded files.

## Building and Running

### Prerequisites
- [Rust](https://rustup.rs/) (2024 edition)
- [Node.js](https://nodejs.org/) or [Bun](https://bun.sh/)

### Development

**Backend:**
```bash
# Start the Rocket server with info logging
RUST_LOG=info cargo run
```

**Frontend:**
```bash
cd web
bun install
bun dev
```

### Production Build
```bash
# Build the frontend assets
cd web && bun run dist
cd ..
# Build and run the optimized backend
cargo run --release
```

## Development Conventions

- **Language**: All code and comments MUST be in **English (US)**.
- **Backend**:
    - Follow standard Rust idiomatic patterns.
    - Use `RUST_LOG` environment variable for log level control.
    - Ensure all file paths are normalized via `config::Folio::normalize_and_join`.
- **Frontend**:
    - Use Functional Components with React Hooks.
    - Styling is handled via **Tailwind CSS 4** and **Radix UI** primitives.
    - Icons are provided by `lucide-react`.
- **Testing**:
    - Run backend tests with `cargo test`.
    - The `src/test_utils.rs` provides helpers for integration testing.

## Configuration

The application is configured via `Folio.toml` or environment variables (prefixed with `FOLIO_`).

| Key | Environment Variable | Default |
|-----|----------------------|---------|
| `web_path` | `FOLIO_WEB_PATH` | `./web/dist` |
| `uploads_path` | `FOLIO_UPLOADS_PATH` | `./uploads` |
| `data_path` | `FOLIO_DATA_PATH` | `./data` |

*Refer to the main `README.md` for detailed Cloudflare Access and API documentation.*
