# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Glance** is a local-first photo timeline browser for photographers. It's designed as a lightweight, modern desktop app for browsing photo archives without replacing professional editing tools like Lightroom or DxO.

**Core positioning:** A Windows desktop app that lets photographers quickly browse their existing photo library by timeline, camera, lens, and other EXIF metadata — without modifying originals or requiring cloud services.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | Tauri (WebView2, ~10MB installer) |
| Backend | Rust |
| Frontend | React + TypeScript |
| Build tool | Vite |
| Database | SQLite (WAL mode) |
| Virtual scrolling | react-virtuoso |
| Image hashing | xxh3 (head 64KB + tail 64KB + file_size); mtime is only for change detection |
| EXIF | kamadak-exif |
| Thumbnail codec | image + fast_image_resize |
| HEIC | libheif (bundled, no system codec dependency) |
| RAW preview | rawler (extract embedded JPEG, no demosaic) |
| Image delivery | Tauri custom asset protocol (`asset://`), not base64/HTTP |

## Architecture

```
Glance/
├── src-tauri/              # Tauri shell + Rust core
│   ├── src/
│   │   ├── main.rs         # Entry point, command registration
│   │   ├── commands/       # Tauri commands (thin orchestration layer)
│   │   └── core/
│   │       ├── scanner/    # Directory traversal, incremental scan
│   │       ├── identity/   # hash + size file identity, mtime change detection
│   │       ├── exif/       # EXIF + XMP sidecar extraction
│   │       ├── thumbnail/  # 3-tier thumbnail generation (240/480/1080px)
│   │       ├── raw/        # Embedded JPEG extraction from RAW
│   │       ├── db/         # SQLite + migrations
│   │       └── tasks/      # Background task queue (IO pool + CPU pool)
│   └── Cargo.toml
├── src/                    # React + TypeScript frontend
│   ├── views/              # Timeline, Lightbox, LibrarySetup
│   ├── components/
│   ├── ipc/                # Tauri command wrappers
│   ├── state/              # Global state + query cache
│   └── styles/
└── Docs/                   # Design docs (Chinese)
```

**Module boundary principle:** Core modules communicate via plain data structures, no cross-imports. `commands/` is the sole orchestrator.

## Key Design Decisions

**File identity:** Uses `content_hash + file_size` as physical file identity on `photo_files`, where `content_hash` is `xxh3(head 64KB + tail 64KB + file_size)`. `mtime` is only a change-detection hint. Schema includes `library_id` from day one for future multi-library support.

**Two-layer schema:** `photos` is the logical timeline entity; `photo_files` stores physical instances. A same-directory RAW+JPEG pair with the same filename stem is one photo: JPEG is `display`, RAW is `raw`, XMP is `sidecar`.

**Thumbnail strategy:** Generate all three tiers (240px, 480px, 1080px short edge) during MVP indexing, stored as WebP at `thumbs/{tier}/{hash[0:2]}/{hash}.webp` using the display source hash. EXIF Orientation is baked in. Non-sRGB images are converted to sRGB with embedded ICC profile. If a cache is missing and the original is unavailable, return a placeholder.

**Data transfer:** Frontend loads thumbnails via `asset://thumb/{photoId}/{tier}` protocol, not base64 or HTTP server. The backend resolves the photo to its cached display source hash.

**Missing-file resilience:** Glance does not auto-classify missing paths as moved, duplicated, deleted, or offline. It marks the original file as missing, shows cached previews when available, and requires manual relocation confirmation. SMB disconnection does not trigger thumbnail cache deletion.

## Local Storage Layout

```
%APPDATA%/Glance/
├── index.sqlite          # Main DB (WAL mode)
├── thumbs/               # WebP thumbnails, bucketed by hash[0:2]
│   ├── 240/
│   ├── 480/
│   └── 1080/
├── config.json
└── logs/
```

## Build Commands

*No source code exists yet — commands will be added as development begins.*

Expected once implemented:
```bash
# Install dependencies
pnpm install          # or npm install

# Development
pnpm tauri dev        # Start Tauri dev server

# Build
pnpm tauri build      # Build production installer

# Rust tests
cd src-tauri && cargo test

# Frontend tests
pnpm test
```

## Development Status

This project is in **pre-development / design phase**. The `Docs/` directory contains detailed design and architecture documents (in Chinese) that define the planned implementation. Key docs:
- `Docs/glance_design_document.md` — Product vision, feature scope, MVP definition
- `Docs/glance_architecture.md` — Technical architecture, schema, data flows

## MVP Scope

The initial release targets:
- Windows installer
- Add local or NAS photo directories
- Local indexing with `hash + size` file identity; `mtime` is only for change detection
- EXIF metadata extraction + XMP sidecar (Rating/Label)
- RAW+JPEG pairing as one timeline photo, with JPEG used for thumbnails/previews
- Three-tier thumbnail generation with Orientation baking and sRGB conversion
- Timeline browsing with virtual scrolling, supporting waterfall and grid views
- Year/month quick jump
- Large image preview
- Read-only library mode
- Missing-file handling (NAS unreachable, moved, deleted, or duplicated paths → show cached preview and require manual relocation)

## Language Notes

Design documents are written in Chinese. When referencing them, preserve the original language in quotes or summaries.
