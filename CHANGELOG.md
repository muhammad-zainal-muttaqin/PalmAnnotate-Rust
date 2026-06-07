# Changelog

All notable changes to PalmAnnotate-Rust. The goal of the project is a **drop-in
behavioral replacement** for the working `PalmAnnotate-Android` (Capacitor + JS) app.

Status legend: ✅ verified on device · 🟡 implemented, pending device confirmation ·
⛔ blocked / needs hardware.

## 2026-06-07 — Launch crash, camera preview, and back navigation

This round fixed the two bugs that made the app unusable in the field, plus a
navigation parity bug. All three were found by running the real APK on the Xiaomi
Pad 6 (`5aa23bd6`) over adb, not from static inspection.

### Fixed

- ✅ **App crashed instantly on launch (SIGABRT).** The size-optimization commit
  enabled R8 minification (`isMinifyEnabled = true`) for both debug and release,
  but `proguard-rules.pro` only protected Orbbec. R8 stripped/renamed the
  Tauri-generated `MainActivity.getId()` and the wry/tao/plugin classes that Rust
  calls via JNI reflection → `java.lang.NoSuchMethodError` before the first frame.
  Added keep rules for `dev.sawitulm.**`, `app.tauri.**`, `@Command` methods, and
  native method names. The earlier "working" screenshots predated this regression.
  - `src-tauri/gen/android/app/proguard-rules.pro`

- ✅ **Camera live preview never appeared ("Camera ready", black/empty view).** The
  device-camera path relied on a native CameraX `ImageAnalysis` → JPEG → base64 →
  `camera-preview` event pump whose frames never reached the WebView listener.
  Replaced it with the JS app's proven approach: `getUserMedia` → `<video>` live
  preview + `<canvas>` grab → JPEG. The Android `RustWebChromeClient` already
  grants the WebView camera permission, and Tauri's `localhost` origin is a secure
  context, so getUserMedia works. Confirmed live preview on device.
  - New backend command `camera_save_frame` (base64 + canvas dims → temp file →
    `CapturedFrame`), wired into the existing commit/cleanup pipeline.
  - `src/capture.rs`, `src-tauri/src/lib.rs`, `Cargo.toml` (web-sys features),
    `src-tauri/Cargo.toml` (base64). Orbbec keeps its native RGB-D pump.

- 🟡 **Back gesture/button closed the whole app instead of navigating.**
  `TauriActivity` sets `handleBackNavigation = false`, so Android's default
  `finish()` ran on every back press. `MainActivity` now registers an
  `OnBackPressedCallback` that calls the in-app `window.__paBack()` and only exits
  when it returns `"exit"` (i.e. already on Home). The Dioxus app exposes
  `__paBack`, mapping each screen to its logical parent (work pages → Session
  detail → Home). Code in place; on-device gesture pass still to be re-run.
  - `src-tauri/gen/android/app/src/main/java/.../MainActivity.kt`, `src/app.rs`

### Verified on device (2026-06-07)

- ✅ App launches to Home (totals, New Session, export-folder row, recent sessions).
- ✅ Capture screen renders the `<video>` element; "Open" starts a live camera feed.
- ✅ Rust workspace tests pass (32 core tests); `cargo check` clean for UI (wasm),
  backend (host); release Android APK builds (~28 MiB, arm64-only).

## Earlier (pre-2026-06-07, uncommitted groundwork carried in this commit)

- 🟡 Global SAF export folder persisted in `AppSettings` (`settings_get/save`),
  New-Session gating, bootstrap validation. (parity with JS "Export folder")
- 🟡 Compute split from export: `tree_compute` computes only; `tree_export` does
  per-type exports (`output`/`yolo`/`csv`/`session`/`identity`/`all`), wired in the
  Results UI — matches the JS separate-buttons model.
- 🟡 `app.rs` split into `capture.rs`, `annotate.rs`, `workflows.rs`.
- 🟡 SAF-mirror IPC arg fix (commands take a single `payload`), asset protocol
  enabled for `convertFileSrc` image rendering, per-side cache-bust token.
- 🟡 annotlog sidecars, delete cascade (incl. annotlog + SAF mirror), GPS via
  `tauri-plugin-geolocation`, quality checks at parity.

See `STATUS.md` for the full implemented-vs-pending matrix and `HANDOFF.md` for the
on-device verification guide.
