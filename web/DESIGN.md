---
version: alpha
name: Folio
description: A calm, trustworthy file-upload interface. Folio combines Arc's expressive, soft display typography with Paws & Paths' light tonal surfaces, generous spacing, and rounded utility components.
colors:
  primary: "#2563EB"
  primary-hover: "#1D4ED8"
  primary-soft: "#EFF6FF"
  primary-border: "#BFDBFE"
  primary-focus: "#3B82F6"
  on-primary: "#FFFFFF"
  background: "#FFFFFF"
  surface: "#FFFFFF"
  surface-subtle: "#EFF6FF"
  surface-translucent: "rgba(255, 255, 255, 0.60)"
  text: "#0F172A"
  text-secondary: "#475569"
  text-muted: "#64748B"
  border: "#CBD5E1"
  border-subtle: "#E2E8F0"
  success: "#059669"
  success-surface: "#ECFDF5"
  success-border: "#A7F3D0"
  on-success-surface: "#047857"
typography:
  display:
    fontFamily: "Marlin Soft SQ, InterVariable, Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: 2.25rem
    fontWeight: 700
    lineHeight: 1.15
    letterSpacing: "-0.02em"
  headline:
    fontFamily: "Marlin Soft SQ, InterVariable, Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: 1.5rem
    fontWeight: 800
    lineHeight: 1.25
    letterSpacing: "-0.01em"
  title:
    fontFamily: "Marlin Soft SQ, InterVariable, Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: 1.125rem
    fontWeight: 700
    lineHeight: 1.4
  body-md:
    fontFamily: "InterVariable, Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: 1rem
    fontWeight: 400
    lineHeight: 1.5
  body-sm:
    fontFamily: "InterVariable, Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: 0.875rem
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "InterVariable, Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: 0.875rem
    fontWeight: 800
    lineHeight: 1.25
    letterSpacing: "0em"
  code:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
    fontSize: 0.875rem
    fontWeight: 700
    lineHeight: 1.5
rounded:
  sm: 4px
  DEFAULT: 8px
  md: 12px
  lg: 16px
  xl: 24px
  upload: 24px
  full: 9999px
spacing:
  compact: 4px
  control: 12px
  default: 16px
  group: 24px
  roomy: 40px
  section: 64px
  gutter: 16px
components:
  primary-action:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.lg}"
    padding: "{spacing.control} {spacing.default}"
    height: 44px
  primary-action-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.lg}"
  upload-dropzone:
    backgroundColor: "{colors.surface-translucent}"
    textColor: "{colors.text}"
    typography: "{typography.body-md}"
    rounded: "{rounded.upload}"
    padding: "{spacing.roomy}"
  upload-dropzone-hover:
    backgroundColor: "{colors.surface-subtle}"
    textColor: "{colors.text}"
    rounded: "{rounded.upload}"
  upload-icon-tile:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.primary}"
    rounded: "{rounded.xl}"
    size: 80px
  file-link:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.primary-hover}"
    typography: "{typography.code}"
    rounded: "{rounded.md}"
    padding: "{spacing.control} {spacing.default}"
  copy-button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.md}"
    size: 48px
  copy-button-hover:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.primary-hover}"
    rounded: "{rounded.md}"
  success-notice:
    backgroundColor: "{colors.success-surface}"
    textColor: "{colors.on-success-surface}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.xl}"
    padding: "{spacing.default}"
---

## Overview

Folio is a single-purpose file-upload interface: the primary task must remain obvious, low-friction, and trustworthy. The visual system uses Arc's soft, expressive typography for hierarchy and Paws & Paths' light surfaces, rounded forms, and spacious utility layout. It is not a browser-brand imitation: blue communicates the next action, while slate carries all reading and structural weight.

## Colors

- **Primary blue:** `primary` is reserved for the upload CTA, active drag state, links, and focus indication. Use `primary-hover` only for hover or pressed feedback.
- **Neutral surfaces:** White is the default canvas. `surface-subtle` and `primary-soft` create quiet grouping and interaction feedback without competing with content.
- **Slate text:** Use `text` for headings, `text-secondary` for descriptions and controls, and `text-muted` for supporting metadata. Do not use blue for normal body copy.
- **Success green:** Use the emerald tokens exclusively for completed-upload confirmation and success state. Do not use green as a second action color.
- **Borders:** Use `border` for the default dashed drop zone and `border-subtle` for quiet card boundaries. Use `primary-border` only around blue interaction states.

## Typography

Arc's type direction is preserved as a hierarchy rule: Marlin Soft SQ is the preferred display and heading face; InterVariable is the utility and body face. Marlin Soft SQ is not included in this repository, so implementations must retain the declared fallback stack until a licensed webfont is provided.

- Use `display` only for the page's principal message.
- Use `headline` for the upload drop-zone title and important state titles.
- Use `title` for compact cards and secondary section headings.
- Use `body-md` and `body-sm` for instructions and lifecycle information.
- Use `code` only for generated download URLs.

## Layout

Use a centered, single-column composition with generous whitespace. Follow Paws & Paths' 8px rhythm: use `control` and `default` inside controls, `group` between related blocks, and `roomy` or `section` between independent sections.

Keep the upload action within the first viewport on standard laptop screens. The main content region should remain narrow enough to make the drop zone feel intentional rather than like a generic dashboard panel.

## Elevation & Depth

Use tonal layering before shadows. Cards and drop zones are defined by white or subtle-blue surfaces and a thin border. Shadows, when needed, must be soft and low-opacity blue, used only to reinforce active drag, hover, or a raised success card. Avoid dark, hard, or multi-layered shadows.

## Shapes

Paws & Paths' rounded language applies throughout Folio:

- Use `rounded.upload` for the primary upload region.
- Use `rounded.xl` for success cards and prominent surface containers.
- Use `rounded.md` for controls, generated-link fields, and icon buttons.
- Use `rounded.full` only for status pills.
- Do not introduce sharp cards, pill-shaped primary buttons, or mixed corner-radius values without a component-level reason.

## Components

- `primary-action` is the sole high-emphasis action. Its minimum height is 44px.
- `upload-dropzone` is the task anchor: use a dashed `border` by default and blue surface/border feedback during drag-over or keyboard focus.
- `upload-icon-tile` gives the drop zone a calm focal point without becoming an additional CTA.
- `file-link` is a read-only presentation of a generated URL; retain monospace type and truncation rather than allowing layout overflow.
- `copy-button` is a 48px square touch target and must retain a visible keyboard-focus ring using `primary-focus`.
- `success-notice` and `status-success` communicate completed work only; they must not be used for general decoration.

## Do's and Don'ts

### Do

- Preserve a single blue primary action per state.
- Keep slate as the default text system and emerald as the success-only semantic color.
- Use generous rounded containers and low-contrast blue surface changes for interaction feedback.
- Keep keyboard focus visible with a blue focus ring and maintain a minimum 44px interactive target.
- Validate new foreground/background combinations against WCAG AA.

### Don't

- Do not use Arc's cream/yellow surfaces, saturated full-page blue fields, or decorative wave treatments.
- Do not use blue for paragraphs, default borders, or all headings.
- Do not add a competing purple, amber, or green CTA.
- Do not replace the declared font fallbacks with an unavailable font name only.
- Do not make the primary upload route visually secondary to status, illustration, or decoration.
