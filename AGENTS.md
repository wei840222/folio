# Project Guidelines for AI Agents and Developers

This document provides a high-level overview of the `folio` project structure, emphasizing the backend API and the frontend application architecture.

## Overview
Folio is a lightweight file storage server featuring a Rust backend and a SvelteKit frontend (located in the `web` directory). It handles file uploads, custom file paths, expiry sweeping, and optional Cloudflare Access JWT protection.

## Backend (Rust)
The backend is a Rust-based Rocket application. It provides the following core endpoints:

### API Endpoints
- **`POST /uploads`**
  - **Purpose**: Upload a file and auto-generate an 8-character ID filename.
  - **Parameters**: `file` (multipart/form-data), `expire` (query parameter, e.g., `10s`, `24h`).
  - **Response**: `201 Created` with a `Location` header pointing to `/files/<generated-name>`.

- **`GET /files/:path`**
  - **Purpose**: Download file content.
  - **Behavior**: Returns `200 OK`, or `302 Found` (redirects to `/private-files/:path`) if the file is private.

- **`GET /private-files/:path`**
  - **Purpose**: Read private file content.
  - **Authentication**: Requires a valid `Cf-Access-Jwt-Assertion` header. Validates JWT signature, issuer, audience, and expiry, along with optional email/group allowlists.

- **`POST /files/:path`**
  - **Purpose**: Create a file at an explicit path.
  - **Response**: `201 Created` or `409 Conflict` if the file already exists.

- **`PUT /files/:path`**
  - **Purpose**: Create or overwrite a file at an explicit path.
  - **Response**: `201 Created` (new) or `200 OK` (overwritten).

- **`DELETE /files/:path`**
  - **Purpose**: Delete a file at an explicit path.
  - **Response**: `200 OK` or `404 Not Found`.

## Frontend (`web` directory)
The frontend is a Single Page Application (SPA) built to generate static pages that interact with the Rust backend.

### Tech Stack
- **Framework**: SvelteKit (`@sveltejs/adapter-static` for static HTML export)
- **Build Tool**: Vite
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **UI Components**: `shadcn-svelte`
- **Icons**: `lucide-svelte`
- **Package Manager**: Bun (`bun.js`)
- **Linting**: ESLint

### Architecture Notes
- **Static Export**: The application is built using `@sveltejs/adapter-static`, generating static HTML/JS/CSS assets that the backend can serve from the configured `FOLIO_WEB_PATH` (defaulting to `./web/build`).
- **Development**: During development (`bun run dev`), Vite proxies API requests (`/uploads` and `/files`) to the Rust backend running on `localhost:8000`.
- **UI Component Library**: Components from `shadcn-svelte` (such as Buttons, Cards, Inputs, and Labels) should be placed in the appropriate `src/lib/components/ui/` directory and utilized throughout the application to maintain consistent styling.
- **State Management**: Svelte's reactive stores and variables should be used for managing upload states (e.g., uploading progress, generated short URLs).
