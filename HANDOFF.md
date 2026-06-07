# PalmAnnotate-Rust — Handoff & On-Device Verification Guide

This document lists everything another agent (or human) needs to **verify whether the
Rust app actually works on the device**, the bugs already fixed, my open hypotheses
(*prasangka*), the still-pending parity work, and the UI rework that remains.

The goal of the project: make **PalmAnnotate-Rust** (Tauri 2 + Dioxus + Rust core) a
**drop-in behavioral replacement** for the working **PalmAnnotate-Android** (Capacitor +
vanilla JS) app. UI does **not** need to look identical — it should be **simple** and may be
**unique to Rust/Dioxus** — but **features and workflow must match**.

Reference (source of truth for behavior): `../PalmAnnotate-Android/` (esp. its `CLAUDE.md`,
`js/`, `css/`).

---

## 0. TL;DR status

> **Update 2026-06-07 (this round):** see `CHANGELOG.md` and `STATUS.md` for the current
> snapshot. Two blockers that made the app unusable were found by running the real APK on the
> Xiaomi Pad 6 and **fixed + verified**: (1) the app crashed instantly on launch — R8 had
> stripped the JNI-called `MainActivity.getId()` (ProGuard keep rules added); (2) the camera
> live preview never showed — replaced the fragile native CameraX preview pump with the JS
> app's `getUserMedia` → `<video>` approach (live feed confirmed on device). Also fixed: the
> Android back gesture closed the whole app (now bridged to in-app `__paBack` navigation).
> The notes below predate this round and are kept for history.

- The app is **NOT "nothing works"** — on-device screenshots show: create session, CameraX
  capture of 4 sides (3264×2448), full tab navigation, workflow rail, local save all working.
- Two concrete bugs were found from the device screenshots and **fixed in code this round**
  (NOT yet verified on device): (A) SAF-mirror arg bug (red banner), (B) images not rendering.
- Pending behavioral parity: annotlog (verify), cacheBust (verify), **Settings page (still a
  stub)**, export trigger, GPS, delete-cascade.
- On-device hardware paths (CameraX edge cases, Orbbec, SAF write, GPS) can ONLY be confirmed
  on the **Xiaomi Pad 6** + an Orbbec Gemini 335L on a powered USB hub.

---

## 1. Build & deploy

Toolchain (this machine, verified):
- JDK 17: `JAVA_HOME` is `C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot\`
- Android SDK: `C:\tools\android-sdk` (platform 34, build-tools 34.0.0)
- Android NDK: `C:\tools\android-sdk\ndk\28.2.13676358`
- Rust 1.93.1, target `aarch64-linux-android`; tauri-cli 2.11.2; Dioxus CLI (`dx`) 0.6.
- ⚠️ Disk is tight. `target/` grows to ~11 GB and the release build briefly needs it. Run
  `rm -rf target` after building if space is low (it was at <2 GB free during this work).

Build the release APK (arm64, debug-signed → installs directly):
```bash
cd PalmAnnotate-Rust
export ANDROID_HOME="C:\tools\android-sdk"
export ANDROID_SDK_ROOT="C:\tools\android-sdk"
export NDK_HOME="C:\tools\android-sdk\ndk\28.2.13676358"
export ANDROID_NDK_HOME="C:\tools\android-sdk\ndk\28.2.13676358"
cargo tauri android build --apk --target aarch64
```
Output: `src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`
(~67.5 MB, arm64-v8a only).

Fast UI-only iteration (no NDK, ~15 s): `dx build` (compiles the Dioxus WASM only). Use this
to catch `app.rs` compile errors before the slow Android build. `cargo check -p palmannotate`
validates `tauri.conf.json` + capabilities. `cargo check -p palmannotate-core` runs the core.

App id / namespace: `dev.sawitulm.palmannotate.rust` (note the `.rust` suffix → installs
**alongside** the JS app `dev.sawitulm.palmannotate`, not over it).

---

## 2. How to actually debug on the device (REQUIRED for verification)

`adb` on this machine shows **no device** — the tablet must be connected with USB debugging.
Once connected:
```powershell
$adb = 'C:\tools\android-sdk\platform-tools\adb.exe'
& $adb install -r "<apk path>"
& $adb shell am force-stop dev.sawitulm.palmannotate.rust
& $adb shell monkey -p dev.sawitulm.palmannotate.rust -c android.intent.category.LAUNCHER 1
# Screenshot:
& $adb shell screencap -p /sdcard/pa.png ; & $adb pull /sdcard/pa.png "$env:TEMP\pa.png"
# Native + Rust logs (panics, Tauri errors):
& $adb logcat -s RustStdoutStderr Tauri PalmNative chromium *:E
```
Inspect the WebView (JS console / invoke errors — best signal for IPC failures):
`chrome://inspect` on the host → the WebView target → Console + Network. Or via CDP:
```powershell
$pid = (& $adb shell pidof dev.sawitulm.palmannotate.rust).Trim()
& $adb forward tcp:9222 localabstract:webview_devtools_remote_$pid
# GET http://localhost:9222/json/list → webSocketDebuggerUrl → Runtime.evaluate
```

---

## 3. Confirmed WORKING on device (from screenshots, 2026-06-07)

- Boot + `bootstrap` IPC (home renders, store path `/data/user/0/dev.sawitulm.palmannotate.rust/PalmAnnotate`).
- New Session form → **session created** ("Damimas / A21B", 4 sides, next id 0002).
- **CameraX capture** of 4 sides at 3264×2448 (Review shows all four).
- Tab navigation (Sessions/Capture/Review/Annotate/Dedup/Results/Depth/Settings).
- Session detail + workflow rail + tree list ("DAMIMAS_A21B_0001 annotated").
- Depth preview renders a gradient (depth_render IPC returns an image).
- Local save works (tree persisted, status reached "annotated").

So Tauri IPC, the core store, CameraX, and the Dioxus UI event loop are all functioning.

---

## 4. Bugs found from device + FIXED this round — **MUST be re-verified on device**

### (A) SAF mirror failed on every screen — red banner
Banner: `Saved locally, but SAF mirror is incomplete: invalid args 'payload' for command
'saf_copy_from_path': command saf_copy_from_path missing required key payload`.

Root cause: the native plugin commands take a single param named `payload`
(`fn saf_copy_from_path(app, payload: SafCopyRequest)`), so Tauri expects invoke args shaped
`{ payload: {...} }`, but `app.rs` sent the fields **flat** (`{treeUri, relativePath, ...}`).

Fixed (wrapped under `payload`) in `src/app.rs`:
- `copy_to_saf` → `saf_copy_from_path`
- `delete_from_saf` → `saf_delete`
- `delete_temporary_frames` → `temp_delete`
- `import_saf_folder` → `saf_copy_tree_to_temp`

**VERIFY:** capture a tree with a SAF export folder set → banner is gone AND files appear in
`<chosen folder>/PalmAnnotate/dataset/...` (check with a file manager or `adb`).

### (B) Captured images render as black boxes (can't annotate)
Root cause hypothesis (high confidence): the `asset:` protocol used by `convertFileSrc` was
**not enabled**. There was no `assetProtocol` in `tauri.conf.json` and the `tauri` crate
lacked the `protocol-asset` feature, so the WebView could not load `convert_file_src(...)`
URLs.

Fixed:
- `src-tauri/tauri.conf.json` → `app.security.assetProtocol = { enable: true, scope: ["**"] }`
- `src-tauri/Cargo.toml` → `tauri = { features = ["protocol-asset"] }`

**VERIFY:** after rebuild, Review/Annotate show the real photos (not black). If still black,
see hypotheses §6.

---

## 5. Pending behavioral parity work (not finished)

| Item | State | What to do / verify |
|---|---|---|
| **annotlog** | backend writer added (`storage.rs save_tree` → `dataset/annotlog/{split}/{tree}_{side}.json`); UI sets `original_bboxes` on detector run | Verify the sidecar files are written on device and contain non-empty `suggestions` after running the detector, and `final` after editing. |
| **cacheBust** | added: token stamped per side in `capture_commit`; appended `?v=` to image URLs in `app.rs` | Verify reusing a tree id does NOT show a stale cached photo. Confirm `?v=` doesn't break the asset URL (see §6). |
| **Settings page** | **NOW WIRED** (`fn Settings`): "Choose folder" → `pick_saf_folder()` (shows chosen folder), "Check permission" → `camera_status` (shows granted/not), "Refresh" → `orbbec_status` (shows device count). | Verify each on device. NOTE: the chosen SAF folder here is **not yet persisted globally** (sessions still set their own `export_uri`); add global persistence if the JS "Export folder" global setting must match. |
| **Export trigger** | Rust auto-exports ALL files inside `tree_compute` (`write_tree_exports`); JS uses separate manual buttons (exportYolo/JSON/CSV/identity/yoloWithMismatch). | Decide if auto-on-compute is acceptable parity (same files produced) or add explicit per-type export actions to match JS UX. |
| **Quality checks** | At parity (Rust `quality.rs` covers variety/block/timestamp/operator/GPS-missing/GPS-low-accuracy>25m/side-count/view/empty/depth/links/unassigned/class-mismatch). Messages are English by design. | Spot-check codes against `js/quality-check.js`. JS also had `metadata_tree_id_missing` (N/A in Rust since tree id always exists). |
| **Delete cascade** | `delete_tree` recurses `dataset/`, `Output TXT/`, `snapshots/` by tree name (now also removes annotlog) + Output JSON + SAF mirror via `delete_from_saf`. | Verify Delete Tree/Session removes images, depth sidecars, Output JSON/TXT, annotlog, snapshots, and SAF-mirrored files. |
| **GPS** | `optional_gps()` uses `tauri-plugin-geolocation` (request perms + 15 s timeout). | Verify the location permission prompt appears on first capture and coords are stored in tree metadata. |

---

## 6. My open hypotheses / suspicions to check (*prasangka*)

1. **Image display may still fail** even with assetProtocol on. On Android, `convertFileSrc`
   maps to `http://asset.localhost/<path>`. Things to check if images stay black:
   - Does the `scope: ["**"]` actually cover the absolute app-data path
     `/data/user/0/dev.sawitulm.palmannotate.rust/PalmAnnotate/...`? Try a more specific scope
     or confirm in the WebView Network tab what URL the `<img>` requests and its status code.
   - The **cacheBust `?v=` query** — confirm the asset handler doesn't 404 on the query string.
     If it does, strip the query for the asset URL or move the bust into the path.
   - Review uses temp-file paths (`convert_file_src(&frame.path)`); Annotate uses the dataset
     path. Verify BOTH resolve (they're different directories → both must be in scope).
2. **Not all payload-commands are exercised** — I only wrapped the 4 the UI currently calls.
   `saf_write`, `saf_exists`, `saf_list`, `saf_read_to_temp`, `saf_release_folder` also take
   `payload`. If any code path calls them, it must wrap args the same way. Grep `app.rs` for
   `plugin:palm-native|` before shipping new flows.
3. **SAF write actually persisting** — the banner fix removes the *error*, but verify the
   Kotlin `saf_copy_from_path` truly writes into the chosen tree (DocumentFile permissions can
   silently fail). Check the chosen folder on-device.
4. **Orbbec** shows "Not attached" — untested. Needs the Gemini 335L on a powered USB hub;
   watch for `data_role=host` drops on PD charge-through (documented in the JS `CLAUDE.md`).
5. **CameraX permission lifecycle** — capture worked once; verify stop/reopen and first-run
   permission denial/grant flow.
6. **Process restart / resume** — verify sessions + trees reload after a full app kill.
7. **annotlog baseline** — `original_bboxes` is only set when the **in-app detector** runs. If
   the operator draws boxes manually without running the detector, `suggestions` will be empty
   (matches JS, which also seeds from detector output — confirm this is the intended parity).

---

## 7. UI rework still needed (lower priority than function)

UI may be **unique to Rust/Dioxus** but must stay **simple** (KISS), per the operator. Done so
far: fixed the stylesheet-404 (was rendering as unstyled plain HTML); simplified Home to
stat-cards + New Session + recent list; shrank oversized headings/paddings app-wide; removed
decorative "eyebrow" labels + verbose descriptions on all pages; hid the topbar on Home.

Still to do (from the latest device screenshots):
- DONE: `work-layout` is now a single column (title + actions on a top row, content panel
  full-width below) — fixes the half-empty left column on Capture/Review/Dedup/Depth/Results.
- `annotation-layout` / `annotation-workspace` (Annotate) still use a fixed multi-column grid
  and may show gaps on tablet — verify and stack similarly if needed.
- Tablet 3:2 11" is **HIGH priority**; phone 9:16 6" is low priority (phone has minor
  horizontal overflow to clean up).
- Keep removing chrome that doesn't earn its space (e.g. the persistent device-status strip,
  schema pill) — match the JS app's information density.

---

## 8. Full on-device acceptance checklist (close these to call it a drop-in)

- [ ] SAF folder pick + persist; create session blocked without it.
- [ ] 4-side AND 8-side capture complete (CameraX).
- [ ] Captured images display in Review and Annotate (not black).
- [ ] SAF mirror writes dataset/metadata/Output into the chosen folder (no red banner).
- [ ] Detector runs; boxes enter as UNASSIGNED (U); assign B1–B4; remove false boxes.
- [ ] annotlog sidecar written with suggestions vs final.
- [ ] Dedup: confirm adjacent-side links (incl. wrap-around); persists.
- [ ] Compute: unique bunches via union-find + majority vote; exports JSON v4 + YOLO (+CSV+identity).
- [ ] Delete tree/session removes ALL artifacts incl. SAF mirror + annotlog.
- [ ] GPS permission prompt + coords stored.
- [ ] Orbbec preview/capture + detach/reconnect recovery (needs hardware).
- [ ] Survives full app restart (sessions/trees reload).
- [ ] CameraX stop/reopen + permission denial/grant.

---

## 9. Key files

- UI + all IPC calls: `src/app.rs` (single Dioxus file).
- Styles: `assets/styles.css` (loaded via `asset!()` + `document::Stylesheet`).
- Tauri backend + commands: `src-tauri/src/lib.rs`.
- Core logic + tests: `crates/palmannotate-core/` (model, storage, dedup, results, output v4,
  quality, detector, depth, yolo; `tests/parity.rs` etc.).
- Native plugin (CameraX/Orbbec/SAF): `plugins/palm-native/` (Rust `src/` + Kotlin
  `android/.../PalmNativePlugin.kt`). Commands take a `payload` param (see §4A).
- Config: `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`,
  `src-tauri/gen/android/app/build.gradle.kts` (release is debug-signed; native symbols stripped).
