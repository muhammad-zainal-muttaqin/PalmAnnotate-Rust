# PalmAnnotate-Rust — Implementation Status

Snapshot: **2026-06-07**. Source of truth for behavior is `../PalmAnnotate-Android`.

Legend: ✅ verified on device · 🟡 implemented, pending device confirmation ·
⛔ needs hardware not currently attached · ❌ not implemented.

## Can it be called a drop-in replacement yet?

**Not yet — but the two blockers that made it unusable are fixed.** The launch
crash and the missing camera preview are resolved and the camera feed is confirmed
live on the Xiaomi Pad 6. Remaining work is verifying the rest of the flow
end-to-end on device (capture→save→review→annotate→dedup→results→export, delete
cascade, GPS, restart) and the Orbbec hardware paths.

## Feature matrix

| Area | State | Notes |
|---|---|---|
| App launches on device | ✅ | ProGuard keep rules restored JNI classes R8 had stripped. |
| Home / Sessions list | ✅ | Totals, New Session, export-folder row, recent sessions, Load Folder/JSON. |
| Session detail | ✅ | Open/Add/Delete tree, Session JSON, Delete session, tree rows. |
| Device camera live preview | ✅ | getUserMedia → `<video>`; live feed confirmed on device. |
| Device camera capture (shoot→save) | 🟡 | Canvas grab → `camera_save_frame` temp file → commit pipeline. Preview verified; full multi-side save/commit to be re-run. |
| 4-side / 8-side capture | 🟡 | Logic present; full pass on device pending. |
| Back gesture / button | 🟡 | `MainActivity` bridges to `__paBack` in-app nav; exits only from Home. Gesture pass to be re-run. |
| Review (swipe, retake, GPS retry, save) | 🟡 | Implemented; device pass pending. |
| Captured images render (not black) | 🟡 | Asset protocol enabled + cache-bust; verify dataset + temp paths on device. |
| Annotate (draw/move/resize/class/delete) | 🟡 | Touch bbox editor implemented; device interaction pass pending. |
| Dedup (adjacent links, wrap-around, suggestions) | 🟡 | Implemented; device pass pending. |
| Results (compute, class mismatch) | 🟡 | Core compute tested (union-find, majority vote); UI pass pending. |
| Exports (YOLO/JSON/CSV/identity) | 🟡 | `tree_export` per-type, wired in Results UI; verify files on device + SAF. |
| Depth viewer | 🟡 | Per-side depth tabs, colorized render; device pass pending. |
| Settings (SAF folder, camera, Orbbec status) | 🟡 | Wired; SAF folder persisted globally in `AppSettings`. |
| Global SAF export folder + New-Session gating | 🟡 | Persisted + bootstrap-validated; device pass pending. |
| SAF mirror writes | 🟡 | IPC `payload` arg fixed; verify files land in chosen folder. |
| annotlog sidecars (suggestions vs final) | 🟡 | Writer present; verify contents on device. |
| Delete cascade (local + SAF + annotlog) | 🟡 | Implemented; verify exact artifacts removed. |
| GPS (permission + coords) | 🟡 | `tauri-plugin-geolocation`, 15 s timeout; manifest perms present. Verify prompt + coords. |
| Survives full app restart (resume) | 🟡 | Store-backed; verify reload after process kill. |
| Orbbec RGB-D (preview/capture/reconnect) | ⛔ | Native pump kept; needs Gemini 335L on a powered USB hub. |
| CameraX stop/reopen + denial/grant | 🟡 | getUserMedia lifecycle; verify deny→grant and re-open. |
| Core data contract (schema v4, YOLO, dedup) | ✅ | 32 core tests pass across 7 suites. |
| APK size / arm64-only | ✅ | ~28 MiB, arm64-v8a, debug-signed. |

## Next on-device pass (to close the matrix)

1. Full capture: New Session (export folder set) → 4 sides via getUserMedia →
   Review shows real photos → Save → tree appears.
2. Back gesture from each screen returns to parent; only Home exits.
3. Annotate: detector run (U boxes) → assign B1–B4 → save → annotlog written.
4. Dedup adjacent links incl. wrap-around persist.
5. Compute + export JSON v4 + YOLO (+CSV+identity); confirm files locally + SAF.
6. Delete tree/session removes all artifacts incl. SAF mirror + annotlog.
7. GPS prompt + coords stored. Restart app → sessions/trees reload.
8. Orbbec (when hardware available).
