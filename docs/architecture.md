# Architecture

## Boundaries

The application is Rust-first. Dioxus owns presentation and local UI state.
`palmannotate-core` owns deterministic domain behavior. Tauri owns privileged
storage and device orchestration. Kotlin is restricted to Android APIs that are
not directly available from Rust.

## Data Flow

```text
CameraX / Orbbec
  -> temporary full-resolution file paths
  -> capture_commit
  -> PalmAnnotate/dataset primary storage
  -> optional SAF mirror
  -> detector / annotation / dedup
  -> Output JSON + Output TXT + exports
```

Preview frames and device status may use Tauri events. Full-resolution RGB,
JPEG, and raw uint16 depth never cross the bridge as base64.

## Core

`crates/palmannotate-core` is platform independent and testable without Android:

- `model`: sessions, trees, sides, bbox, links, GPS, depth metadata, output v4.
- `yolo`: strict B1-B4 parser and six-decimal serializer.
- `results`: union-find clustering, stale-link rejection, majority class count.
- `output`: stable schema v4 and legacy output loading.
- `quality`: deterministic metadata/capture/annotation checks.
- `depth`: uint16 little-endian display filtering, P2-P98 range, and heatmap
  colorization matching the legacy 250-7000 mm viewing contract.
- `storage`: app-data layout, atomic writes, imports, and lifecycle deletion.

## Persistence

The local app-data store is authoritative. Writes use a temporary sibling file
followed by rename. `sessions.json` is the index; each tree also has a canonical
document under `trees/`. Derived schema v4 and YOLO outputs are regenerated on
tree save. Results additionally write CSV, session JSON, identity JSON, and
normal/mismatch YOLO exports under `exports/`.

SAF is a mirror and import boundary. A mirror failure is recoverable and cannot
roll back a successful primary commit. Import preserves source IDs and stops on
conflict.

## Android Plugins

- CameraX: permission, low-resolution preview, full-resolution temporary JPEG,
  stop/reopen, and controlled fallback.
- Orbbec: USB vendor filter, permission, list/refresh, RGB/depth preview,
  synchronized full-resolution capture paths, serialized preview/capture,
  detach, and close.
- SAF: pick/release folder, list, read-to-temp, copy-from-path, write, exists,
  and delete.
- Geolocation: optional GPS with a 15-second timeout.

All plugin operations return typed objects and recoverable errors. Heavy camera,
file, detector, depth-render, and export operations execute off the UI thread.

## Detector

`models/ffb-detector.onnx` and `detector.config.json` are packaged in the app.
The Android backend uses `ort 2.0.0-rc.12` with ONNX Runtime 1.24.x:

1. Decode image from path.
2. Letterbox to 640 x 640.
3. Normalize and arrange NCHW tensor.
4. Execute inference on a worker.
5. Detect output orientation.
6. Apply confidence threshold and NMS.
7. Undo letterbox coordinates.
8. Emit every retained bbox as class ID `-1`, class name `U`.

## UI

The tablet layout uses a stable side rail, broad touch targets, one green accent,
and a strict single-column mobile fallback. It implements loading, empty, error,
disabled, hover, active, and offline-device states.
