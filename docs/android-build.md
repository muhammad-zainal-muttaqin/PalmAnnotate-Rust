# Android Build

## One-time Setup

Run from the repository root:

```powershell
.\scripts\setup-android.ps1
```

The script installs or verifies Rust 1.93.1 targets, Tauri CLI 2.11.x, Dioxus
CLI 0.6.3, a portable JDK 17, Android platform/build tools 34, platform tools,
and NDK `28.2.13676358`.

Set the printed environment variables in the current shell before invoking
Tauri.

## Initialize Generated Android Project

Identity must already be correct in `Cargo.toml`, `Dioxus.toml`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

```powershell
cargo tauri android init
```

Generated Android sources live under `src-tauri/gen/android`. The repository
keeps the generated project because it contains the CameraX, Orbbec, SAF,
manifest, ABI, and vendor AAR integration.

## Debug APK

```powershell
dx build --release
cargo tauri android build --apk --debug --target aarch64
```

Only `arm64-v8a` is supported. Debug signing uses the standard generated debug
keystore. AAB and release signing are intentionally deferred.

## APK Audit

Use Build Tools 35 only for the audit command because its `zipalign` supports
the 16 KB page-size flag; the app still compiles and targets Android SDK 34.

```powershell
$apk = "src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk"
.\.toolchains\android-sdk\build-tools\35.0.0\apksigner.bat verify --verbose --print-certs $apk
.\.toolchains\android-sdk\build-tools\35.0.0\zipalign.exe -c -P 16 -v 4 $apk
```

Extract packaged `.so` files and verify every ELF `LOAD` segment has alignment
`0x4000` with NDK `llvm-readelf -lW`. Also verify `aapt2 dump badging` reports
package `dev.sawitulm.palmannotate.rust`, minimum SDK 24, target SDK 34, and
only `arm64-v8a` native code.

## Required Android Configuration

- namespace/application ID: `dev.sawitulm.palmannotate.rust`
- min SDK: 24
- compile/target SDK: 34
- NDK: `28.2.13676358`
- ABI filter: `arm64-v8a`
- permissions: CAMERA, coarse/fine location, INTERNET
- optional features: camera, autofocus, GPS, USB host
- USB vendor filter: Orbbec `0x2BC5`
- rotation/config changes suitable for tablet field use
- CameraX lifecycle dependencies
- AndroidX DocumentFile for SAF
- Orbbec AAR stripped to arm64 and without unused firmware updater assets

## Physical Verification

Install on Xiaomi Pad 6 and verify:

1. First-run camera/location/USB permissions.
2. Four-side and eight-side capture.
3. Process restart and session resume.
4. Fully offline detector.
5. SAF mirror and import.
6. Tree delete and ID reuse cleanup.
7. Orbbec RGB/depth preview and full capture.
8. USB detach and reconnect.
