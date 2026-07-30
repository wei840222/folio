---
version: 1.0.0
name: Agenfact Design System
description: Cyberpunk / Silicon-Dark UI design specification for Agenfact (Cyber Artifact Hub). Combines JetBrains Mono display typography, Inter body copy, glassmorphic dark containers, laser-sweep animations, and neon cyan/purple/emerald accents.
colors:
  background: "#060913"
  surface-glass: "rgba(13, 20, 36, 0.75)"
  surface-subtle: "rgba(0, 242, 254, 0.05)"
  cyber-cyan: "#00F2FE"
  cyber-purple: "#C084FC"
  cyber-emerald: "#34D399"
  text: "#F8FAFC"
  text-secondary: "#94A3B8"
  text-muted: "#64748B"
  border: "rgba(0, 242, 254, 0.18)"
  border-subtle: "rgba(255, 255, 255, 0.08)"
typography:
  display:
    fontFamily: "'JetBrains Mono', ui-monospace, monospace"
    fontSize: "2.25rem - 3.75rem"
    fontWeight: 800
    letterSpacing: "-0.03em"
  label:
    fontFamily: "'JetBrains Mono', ui-monospace, monospace"
    fontSize: "0.625rem - 0.75rem"
    fontWeight: 700
    letterSpacing: "0.08em"
  body:
    fontFamily: "'InterVariable', 'Inter', ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem - 1rem"
    fontWeight: 400
    lineHeight: 1.6
components:
  uplink-card:
    background: "cyber-glass (backdrop-filter blur 16px)"
    border: "1px solid rgba(0, 242, 254, 0.18)"
    rounded: "24px"
  dropzone:
    border: "2px dashed border-border/60"
    animation: "laser-sweep 2.5s infinite"
    hover: "border-cyber-cyan bg-cyber-cyan/10"
  artifact-preview-modal:
    markdown: ".prose-cyber (Dark prose with cyan code highlights)"
    media: "Image viewport & raw code viewer"
---

## Overview

**Agenfact** is a high-speed, silicon-dark digital artifact uplink interface designed for AI Agents and developers. The UI aesthetic emphasizes a sleek cyberpunk cyber-glass theme (`.silicon-bg` and `.cyber-glass`) with neon accents to make uploaded artifacts immediately visually striking, accessible, and readable.

## Colors & Surfaces

- **Silicon Dark Background (`#060913`):** Dark canvas featuring radial glow gradients in cyan (`#00f2fe`), purple (`#a855f7`), and emerald (`#10b981`), overlaid with a 32px hexagonal/grid pattern.
- **Cyber Glass Surfaces (`.cyber-glass`):** Semi-transparent dark containers (`rgba(13, 20, 36, 0.75)`) with `backdrop-filter: blur(16px)` and subtle cyan border highlights (`rgba(0, 242, 254, 0.18)`).
- **Neon Accents:**
  - **Cyber Cyan (`#00f2fe`):** Primary uplink actions, progress indicators, active dropzone states, and code highlights.
  - **Cyber Purple (`#c084fc`):** Turnstile challenge cards, secondary badges, and markdown report tags.
  - **Cyber Emerald (`#34d399`):** System nominal indicators, upload completion notices, and media badges.
- **Text Palette:** Pure white/slate (`#f8fafc`) for headings, muted slate (`#94a3b8`) for descriptions and labels, and dark slate (`#64748b`) for footers and secondary details.

## Typography

- **Display & Monospace (`JetBrains Mono`):** Used for titles, branding (`AGENFACT`), badge tags, dropzone statuses, code snippets, and URL displays.
- **Body (`InterVariable` / `Inter`):** Used for descriptions, option labels, tooltips, and general copy.

## Interactive Components & Micro-animations

1. **Uplink Core Dropzone (`FileUploadZone`):**
   * Features a dynamic laser line sweep animation (`laser-sweep`) on hover or dragover.
   * Dashed border transition to bright cyber cyan glow on active drop.
2. **Artifact Preview Modal (`ArtifactPreviewModal`):**
   * Supports instant inline rendering of Markdown (`.prose-cyber`), Code artifacts, and UI Mockups/Media.
   * Markdown styling includes dark code blocks with cyan text, bordered blockquotes, and styled list items.
3. **Download Link & QR Modal (`DownloadLink`, `QRCodeModal`):**
   * Instant one-click copy button with visual feedback.
   * Dedicated QR Code modal for quick mobile testing and sharing.
4. **Upload Options (`UploadOptions`):**
   * Collapsible drawer for setting email access restrictions (`authorized_emails`) and expiration TTL (`expire_ttl`).

## Layout & Structure

A responsive single-page layout wrapped in `max-w-6xl` centered container, scaling gracefully from mobile viewports to large desktop monitors (`lg:grid-cols-[1.04fr_0.96fr]`).
