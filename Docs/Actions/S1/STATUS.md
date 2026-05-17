# S1 Stage Status

**Last Updated:** 2026-05-17 (post PR8-PR15 implementation)

## Summary

S1 stage PR1–PR15 are now implemented. All Rust tests pass (75/75), frontend builds successfully, and the end-to-end pipeline is wired: Tauri commands registered, `asset://` protocol active, thumbnails generated during scan, schema aligned with architecture doc.

## Test Results

```
Rust:     75 / 75 passing
Frontend: pnpm build OK
E2E:      pnpm tauri dev — add directory → scan → timeline → lightbox flow wired
```

## PR Status

| PR | Status | Notes |
|---|---|---|
| S1-PR1 | Done | Tauri + React + Rust skeleton |
| S1-PR2 | Done | Schema (realigned in PR10) |
| S1-PR3 | Done | Scanner + identity + pairing (fixed in PR11) |
| S1-PR4 | Done | EXIF + XMP (fixed in PR11) |
| S1-PR5 | Done | Scan orchestration (fixed in PR11/PR12) |
| S1-PR6 | Done | Thumbnail pipeline (fixed in PR9) |
| S1-PR7 | Done | Timeline UI (rewritten in PR14) |
| **S1-PR8** | **Done** | Tauri IPC wiring + asset:// protocol |
| **S1-PR9** | **Done** | Thumbnail integration, lossy WebP, fast_image_resize, ICC→sRGB |
| **S1-PR10** | **Done** | Schema realignment: format→photo_files, library_id, missing_since, indexes |
| **S1-PR11** | **Done** | EXIF Short/Long types, scanner B6/B7/B8/B11 fixes |
| **S1-PR12** | **Done** | Batch transactions (N=200), streaming discovery, cursor-based resumable scan, pause/resume |
| **S1-PR13** | **Done** | RAW embedded JPEG extraction, RAW-only photo support, HEIC feature gate |
| **S1-PR14** | **Done** | GroupedVirtuoso, Lightbox wiring, no per-photo IPC |
| **S1-PR15** | **Done** | Task queue with IO/CPU pools, priority, cancellation |

## What Was Fixed (Review Issues)

| Issue | Status |
|---|---|
| P0: Tauri commands empty | Fixed (PR8) |
| P0: asset:// protocol missing | Fixed (PR8) |
| P0: Thumbnails not generated | Fixed (PR9) |
| B1: EXIF Short/Long types | Fixed (PR11) |
| B2: Lossless WebP | Fixed (PR9) — lossy q=85 via webp crate |
| B3: fast_image_resize unused | Fixed (PR9) |
| B4: HEIC panic | Fixed (PR13) — feature gated |
| B5: ICC/sRGB not done | Fixed (PR9) — qcms integration |
| B6: JPEG misclassified as raw | Fixed (PR10/PR11) |
| B7: finished_at always written | Fixed (PR10) — mark_scan_complete/failed/paused |
| B8: No metadata refresh | Fixed (PR10/PR11) |
| B9: No scan transactions | Fixed (PR12) — batched BEGIN/COMMIT |
| B10: Cursor unused | Fixed (PR12) — cursor updated per batch |
| B11: Sidecar identity match | Fixed (PR10/PR11) |
| B12: format on photos | Fixed (PR10) — moved to photo_files |
| B13: Schema misalignment | Fixed (PR10) |
| F1: Lightbox not opened | Fixed (PR14) |
| F2: VirtuosoGrid misuse | Fixed (PR14) — GroupedVirtuoso |
| F3: Per-photo IPC | Fixed (PR8/PR14) — thumbnail_url in PhotoSummary |

## Architecture Notes

- **Schema**: `photos` has no `format` column; `photo_files` has `library_id`, `format`, `missing_since`; `UNIQUE(library_id, path)`
- **Scan**: Batched transactions (200 candidates/batch), cursor-based resume, pause/resume via `AtomicBool` cancellation
- **RAW**: Embedded JPEG extraction via SOI/EOI marker scanning; RAW-only photos promoted to display role
- **Thumbnails**: Lossy WebP (q=85), fast_image_resize (Lanczos3), ICC→sRGB via qcms
- **Task Queue**: `core::tasks::TaskQueue` with IO pool (3 workers) + CPU pool (num_cpus), priority ordering, cooperative cancellation
