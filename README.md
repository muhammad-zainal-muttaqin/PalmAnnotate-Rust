# PalmAnnotate

PalmAnnotate is an offline Android field application for capturing, annotating,
deduplicating, and exporting multi-view oil palm bunch datasets.

This repository is the Rust/Tauri replacement for `../PalmAnnotate-Android`.
The legacy JavaScript application remains a read-only behavioral reference.

## Scope

- Dioxus 0.6 UI written in Rust.
- Tauri 2 runtime and typed IPC.
- Rust domain model, schema v4 output, YOLO IO, result computation, quality
  checks, local persistence, import, and export.
- Android Kotlin only for CameraX, Orbbec USB, and Storage Access Framework.
- Android debug APK for `arm64-v8a`; desktop packaging, AAB, and release signing
  are outside the migration scope.

See [REQUIREMENTS.md](REQUIREMENTS.md), [docs/architecture.md](docs/architecture.md),
and [docs/android-build.md](docs/android-build.md).

## Repository Layout

```text
assets/                         Dioxus styles and existing UI assets
crates/palmannotate-core/       Platform-independent domain and storage logic
models/                         Embedded ONNX detector and config
scripts/setup-android.ps1       Idempotent toolchain bootstrap
src/                            Dioxus UI
src-tauri/                      Tauri backend and generated Android project
```

## Bootstrap

```powershell
.\scripts\setup-android.ps1
```

The script prints environment exports for the current shell. Apply them, then:

```powershell
$env:JAVA_HOME = "$PWD\.toolchains\jdk-17"
$env:ANDROID_HOME = "$PWD\.toolchains\android-sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\28.2.13676358"
$env:PATH = "$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:ANDROID_HOME\cmdline-tools\latest\bin;$env:PATH"
```

## Verification

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
dx build --release
cargo tauri android build --apk --debug --target aarch64
```

The universal debug APK is generated at:

```text
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Automated verification covers Rust format/lint/tests, the Dioxus release build,
Kotlin compilation, APK signing, arm64-only packaging, 16 KB ZIP/ELF alignment,
manifest identity/SDK values, and model checksum parity. Hardware behavior must
still be verified on the target Xiaomi Pad 6 and Orbbec camera; see the
acceptance checklist in [REQUIREMENTS.md](REQUIREMENTS.md).

## Data Guarantees

- Individual JSON/TXT/export writes are atomic. Capture replacement stages new
  files and restores the prior tree dataset if commit fails.
- A persistent SAF export folder is required before a session can be created.
- SAF mirror failure does not roll back or delete primary data.
- Import rejects conflicting IDs and never overwrites silently.
- Reusing a tree ID requires lifecycle cleanup of its prior dataset, JSON, TXT,
  snapshot, RGB, and depth artifacts before committing replacement data.
- Full-resolution RGB/depth data moves by file path, never as large event
  payloads or base64 over the Tauri bridge.
- Small CameraX and Orbbec RGB/depth previews may use Tauri events.

## License

MIT. See `LICENSE.txt` in the reference project for the original application
license.
