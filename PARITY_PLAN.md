# PalmAnnotate Rust Replacement Plan

## Goal

`PalmAnnotate-Rust` must replace `PalmAnnotate-Android` for field use. The JavaScript
application is the behavioral reference. The Rust application may use different internal
architecture, but the operator workflow, persisted data, exports, and device behavior must
remain equivalent.

## Product Rules

- Keep the interface action-first. Do not add product claims, architecture labels, workflow
  explanations, encouragement, or decorative status text.
- Keep the interface visually compact and keep the install/runtime footprint small enough for
  practical field deployment.
- Keep the primary flow visible: Sessions -> Capture -> Review -> Annotate -> Dedup -> Results.
- Do not expose a page or button until it performs real work.
- Preserve the JavaScript storage contract, schema v4 fields, filenames, class rules,
  adjacency rules, delete behavior, and resumability.
- Treat local app data as the reliable working store and the selected SAF folder as the
  persistent public mirror.
- Hardware-only checks remain explicit, but static code parity, tests, builds, and browser UI
  checks must be complete before requesting device verification.

## Feature Matrix

| Area | JavaScript behavior to preserve | Rust work required | Completion evidence |
|---|---|---|---|
| Export folder | One global persisted SAF folder; required before New Session or Add Tree; can be reselected and used to resume | Add persisted app settings, verify grants, use the selected folder for new sessions, import, mirror, and delete | Restart-safe store tests plus UI routing tests |
| Sessions home | Compact totals, New Session, recent sessions, export-folder row, folder import | Remove sidebar/dashboard chrome and explanatory copy; keep direct actions | Browser screenshots at tablet and phone widths |
| Session detail | Open a tree, add tree, delete tree, delete session, download/export session | Make tree rows resumable and route to the selected tree; preserve full deletion | UI tests and storage deletion tests |
| Capture | CameraX default, optional Orbbec, Find camera, persistent live preview, 4/8 sides, manual ID, cancel cleanup | Add source discovery/refresh and accurate lifecycle states | Kotlin tests/static checks plus Rust UI tests |
| Review | Swipe/tap side navigation, per-side Retake, GPS retry, capture quality summary, Save/Cancel | Preserve accepted sides while retaking one side; expose GPS retry and validation | Component tests covering single-side retake and cancel |
| Annotate | Review/Edit modes, side swipe/tabs, select, draw, move, resize, class B1-B4, delete, boxes toggle, detector rerun, class propagation through links | Replace read-only overlays and list-only editing with a real touch bbox editor | Interaction tests for draw/move/resize/class/delete |
| Dedup | Visual adjacent-side comparison, manual tap-link, remove links, suggestions, accept/reject, bbox class/delete tools | Replace dropdown-only linking with image overlays and direct selection | Interaction tests for wrap-around and endpoint replacement |
| Results | Compute counts, quality, class mismatch, Save Output, YOLO, session JSON, CSV, identity exports | Keep compute separate from explicit export actions while retaining local and SAF writes | Export file tests and UI action tests |
| Depth | Per-side depth tabs, robust range, colorized preview | Use tree side metadata instead of free numeric input | Depth tests and browser rendering |
| Resume/import | Restore sessions and trees from local store or selected SAF folder without overwriting conflicts | Persist settings and make import adopt the selected folder | Import conflict and restart tests |
| Delete/reuse | Remove local and SAF images, depth, metadata, JSON, TXT, annotlog, snapshots, exports, and stale cache entries | Centralize artifact manifests so delete and mirror cover the same paths | Exact artifact deletion tests |
| Orbbec | USB attach/detach notification, refresh after replug, selected profiles, synchronized RGB/depth, D2C alignment, one pipeline reader, ordered teardown | Port the proven lifecycle and profile-selection behavior to the Tauri plugin | Kotlin compile, source parity tests, device checklist |
| CameraX | Permission request, live preview, full-resolution capture, stop/reopen | Keep preview and capture state restartable after denial or cancellation | Kotlin compile and lifecycle source tests |
| Responsive UI | Tablet-first, functional portrait phone, no rotate gate | Compact top controls and single-column reflow where needed | Browser checks at 1366x900, 1024x768, and 360x800 |
| Size/performance | The replacement should not regress into an oversized or wasteful package | Audit APK entries, remove unused ABIs/assets/features, optimize Rust/WASM/native release profiles, avoid unnecessary preview allocations, and lazy-load expensive work | APK size report, ABI/ELF audit, startup/build checks, and comparison with the JS APK |

## Implementation Order

1. Persist global settings and rebuild the shell/session routing around the JavaScript flow.
2. Complete capture discovery, per-side review/retake, GPS retry, and cleanup.
3. Implement the touch annotation editor and class/link propagation.
4. Implement visual dedup, explicit results exports, and side-based depth navigation.
5. Port the proven Orbbec lifecycle, profile selection, alignment, and reconnect behavior.
6. Audit and reduce APK/runtime cost without removing required offline or hardware features.
7. Add parity-focused tests, run all Rust/Kotlin/UI builds, inspect the rendered app, and build
   a fresh arm64 APK.

## Completion Gate

Completion requires every matrix row to have current code evidence and automated verification
where hardware is not required. The final hardware checklist must contain only physical-device
facts that cannot be proven from source, tests, a desktop WebView, or the Android build.

## Verification - June 7, 2026

- Rust workspace: 41 tests passed across 13 suites.
- `cargo check` and strict Clippy (`-D warnings`) passed.
- Android Kotlin/native bridge compilation passed.
- Browser QA passed at 1366x900, 1024x768, and 390x844 with no horizontal page overflow.
- Browser interaction QA covered session creation, CameraX and Orbbec selection, four-side
  capture, GPS, review/save, annotation, dedup suggestions, results compute/export, depth,
  settings, SAF reset, and responsive navigation.
- Final APK: `app-universal-release.apk`, 29,568,592 bytes (28.20 MiB), arm64-only, 625 entries,
  APK Signature Scheme v2 verified, no packaged debug native artifacts.
- Remaining checks require the physical tablet: real CameraX frames, Orbbec USB permission and
  RGB/depth synchronization, SAF grant persistence across Android restarts, and live GPS.
