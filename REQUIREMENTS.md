# PalmAnnotate Rust Requirements

## Product

- Product name: `PalmAnnotate`
- Repository folder: `PalmAnnotate-Rust`
- Android identifier: `dev.sawitulm.palmannotate.rust`
- Target artifact: debug APK, `arm64-v8a`
- Minimum Android SDK: 24
- Compile and target SDK: 34

## Required Field Workflow

1. Select and persist an SAF export folder.
2. Create or resume a field session.
3. Create a tree with a unique ID and 4 or 8 sides.
4. Capture full-resolution RGB with CameraX or synchronized RGB/depth with
   Orbbec.
5. Store capture files in local primary app data and attempt an SAF mirror.
6. Review side count, metadata, image dimensions, RGB/depth pairing, and GPS.
7. Run the embedded offline detector or draw boxes manually.
8. Assign every detector result from `UNASSIGNED` to B1, B2, B3, or B4.
9. Confirm duplicate identities only across adjacent sides.
10. Compute unique bunches, inspect class mismatch, and export JSON v4 plus YOLO.
11. Delete all tree artifacts together when deleting or reusing a tree ID.

## Architecture

- `palmannotate-core`: models, validation, YOLO parser, union-find result
  computation, schema v4, quality checks, atomic persistence, import conflict
  handling, and deterministic tests.
- Dioxus web UI: Home, New Session, Session Detail, Capture, Review, Annotate,
  Dedup, Results, Depth Viewer, and Settings.
- Tauri backend: app-data ownership, typed IPC, worker tasks, file operations,
  capture commit, detector, compute, import, export, and lifecycle deletion.
- Android Kotlin plugins: CameraX, Orbbec, and SAF only.
- Tauri geolocation plugin: optional GPS with a 15-second timeout.

## Toolchain

| Dependency | Required |
|---|---|
| Rust | 1.93.1 stable |
| Rust targets | wasm32-unknown-unknown, aarch64-linux-android |
| Tauri runtime/CLI | 2.11.x |
| Dioxus/CLI | 0.6.3 |
| JDK | 17 |
| Android platform | 34 |
| Android build tools | 34.0.0 |
| Android NDK | 28.2.13676358 |
| ABI | arm64-v8a only |
| CameraX | 1.4.2 |
| ONNX Runtime | 1.24.x |
| ort crate | 2.0.0-rc.12 |

## Storage Layout

```text
PalmAnnotate/
  sessions.json
  dataset/
  Output JSON/
  Output TXT/
  exports/
  snapshots/
  trees/
```

## Stable Data Contract

- Output schema version is `4`.
- New output keys are English.
- Legacy loaders may accept Indonesian labels and older output shapes.
- Stable keys include `_confirmedLinks`, `box_index`, `side_index`, `sideA`,
  `sideB`, `bboxIdA`, and `bboxIdB`.
- Bbox IDs reconstructed from output use `b{box_index}`.
- Unassigned class uses ID `-1` and name `U`; it remains in JSON but is omitted
  from YOLO TXT.
- Detector output is always unassigned regardless of model class prediction.

## Typed IPC

- `bootstrap`
- `session_list`, `session_save`, `session_delete`, `sessions_import`,
  `sessions_import_folder`
- `tree_load`, `tree_save`, `tree_delete`, `tree_suggest`
- `capture_commit`
- `detector_run`
- `tree_compute`, `depth_render`
- Android plugin commands for CameraX preview/capture, Orbbec
  status/list/permission/preview/capture/close, SAF folder/file operations,
  temporary-file cleanup, and geolocation

All failures serialize as:

```json
{
  "code": "machine_readable_code",
  "message": "Operator-facing description",
  "recoverable": true
}
```

## Acceptance Checklist

- [x] No runtime dependency on legacy JavaScript sources.
- [x] App persists and reloads sessions/trees through the Rust store.
- [x] Session creation is blocked until an SAF folder grant exists.
- [ ] Four-side and eight-side capture complete on Xiaomi Pad 6.
- [ ] CameraX preview, capture, stop, and reopen work after permission changes.
- [ ] Orbbec preview/capture works and recovers after USB detach/reconnect.
- [x] Full-resolution RGB/depth files use temporary/local paths across IPC.
- [x] Embedded detector model checksum/config/decode match the legacy model and
  every decoded result is forced to unassigned.
- [x] Schema v4, YOLO, cluster, count, depth-range, quality, export, and
  stable-link behavioral tests pass.
- [x] SAF mirror failure does not delete or roll back local primary data.
- [x] SAF import validates references, preserves IDs, and rejects conflicts.
- [x] Tree delete/reuse removes prior local artifacts and capture rollback
  restores the prior dataset on commit failure.
- [x] Rust format, Clippy, tests, Dioxus release build, Kotlin compile, and
  Android APK build pass.
- [x] APK is debug-signed, arm64-only, and ZIP/ELF aligned for 16 KB pages.

Unchecked hardware items are mandatory before field rollout. They cannot be
closed by emulator or static inspection because they depend on Xiaomi camera
behavior, USB permission/attach callbacks, the Orbbec SDK/device, and actual
process restart on the target tablet.
