# PalmAnnotate-Rust — Build & Setup Guide

How to set up a **fresh machine**, clone the repo, and produce an installable
Android APK (and how to iterate fast). This is the practical companion to
`REQUIREMENTS.md` (which lists the formal version contract) and `HANDOFF.md`
(on-device debugging).

Target output: a debug-signed, directly installable **`arm64-v8a` APK** for the
Xiaomi Pad 6 (Android 14). The app id is `dev.sawitulm.palmannotate.rust` — it
installs **alongside** the JS app (`dev.sawitulm.palmannotate`), not over it.

---

## 1. What you must install

Exact versions this project is built and verified with. Newer minor versions of
the SDK/NDK usually work, but the toolchain channel is pinned in
`rust-toolchain.toml` and should not be changed casually.

| Tool | Version | Notes |
|---|---|---|
| Rust (rustup) | **1.93.1** | Pinned by `rust-toolchain.toml`; rustup auto-installs it. |
| Rust targets | `wasm32-unknown-unknown`, `aarch64-linux-android` | Also pinned in `rust-toolchain.toml`. |
| Tauri CLI | **2.11.2** | `cargo install tauri-cli --version "^2.11"` |
| Dioxus CLI (`dx`) | **0.6.3** | `cargo install dioxus-cli --version "0.6.3"` |
| JDK | **17** (Temurin 17.0.19) | Gradle + Android plugin need JDK 17. |
| Android SDK platform | **android-34** | compile/target SDK 34. |
| Android build-tools | **34.0.0** | |
| Android NDK | **28.2.13676358** | Builds the Rust `.so`. |
| Android cmdline-tools | latest | To install the above via `sdkmanager`. |
| ONNX Runtime | 1.24.x (via `ort` 2.0.0-rc.12) | Pulled by Cargo; on-device detector. |

Minimum Android on device: **SDK 24** (required by the Orbbec SDK).

### 1a. Install Rust + targets + CLIs

```bash
# 1. rustup (https://rustup.rs). Then, inside the repo, the pinned toolchain
#    auto-installs on first cargo command. To be explicit:
rustup toolchain install 1.93.1
rustup target add wasm32-unknown-unknown aarch64-linux-android

# 2. Tauri + Dioxus CLIs (installed into ~/.cargo/bin — put it on PATH)
cargo install tauri-cli --version "^2.11" --locked
cargo install dioxus-cli --version "0.6.3" --locked
```

Verify: `cargo tauri --version` → `tauri-cli 2.11.2`; `dx --version` → `dioxus 0.6.3`.

### 1b. Install JDK 17

Install Temurin/Adoptium JDK 17. It does **not** need to be on `PATH` globally —
the build sets `JAVA_HOME` inline (see §3). On the current dev machine it lives at:

```
C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot
```

### 1c. Install the Android SDK + NDK

Install Android **cmdline-tools**, then use `sdkmanager` to add the packages:

```bash
sdkmanager "platform-tools" \
           "platforms;android-34" \
           "build-tools;34.0.0" \
           "ndk;28.2.13676358"
```

On the current dev machine the SDK is at `C:\tools\android-sdk` and the NDK at
`C:\tools\android-sdk\ndk\28.2.13676358`. `adb` is at
`C:\tools\android-sdk\platform-tools\adb.exe`.

---

## 2. Clone

```bash
git clone https://github.com/muhammad-zainal-muttaqin/PalmAnnotate-Rust.git
cd PalmAnnotate-Rust
```

What's committed and what's generated:

- **Committed (do not delete):** `src/` (Dioxus UI), `crates/palmannotate-core`,
  `plugins/palm-native`, `src-tauri/` **including `src-tauri/gen/android`** (the
  generated Android Studio project is checked in, with our hand-edited
  `MainActivity.kt` and `proguard-rules.pro`), `models/`, `assets/`.
- **Generated / git-ignored (recreated by the build):** `target/`, `dist/`,
  `src-tauri/gen/android/app/build/`, `src-tauri/gen/schemas/`.

> Because `src-tauri/gen/android` is committed, you do **not** run
> `cargo tauri android init` on a fresh clone — that would overwrite the
> hand-edited `MainActivity.kt` (back-navigation bridge) and `proguard-rules.pro`
> (JNI keep rules). Only re-init if you intentionally regenerate the shell, and
> then re-apply those two files.

---

## 3. Build the release APK

The Android build needs the Android env vars set. JDK/SDK/NDK are **not** assumed
to be on `PATH`, so set them inline.

**PowerShell (Windows — current dev machine):**

```powershell
$env:JAVA_HOME        = 'C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot'
$env:ANDROID_HOME     = 'C:\tools\android-sdk'
$env:ANDROID_SDK_ROOT = 'C:\tools\android-sdk'
$env:NDK_HOME         = 'C:\tools\android-sdk\ndk\28.2.13676358'
$env:ANDROID_NDK_HOME = 'C:\tools\android-sdk\ndk\28.2.13676358'
$env:PATH             = "$env:JAVA_HOME\bin;$env:PATH"

cargo tauri android build --apk --target aarch64
```

**bash (Linux/macOS — adjust paths):**

```bash
export JAVA_HOME=/path/to/jdk-17
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_SDK_ROOT=$ANDROID_HOME
export NDK_HOME=$ANDROID_HOME/ndk/28.2.13676358
export ANDROID_NDK_HOME=$NDK_HOME
export PATH="$JAVA_HOME/bin:$PATH"

cargo tauri android build --apk --target aarch64
```

What it does: runs `dx build --release` (compiles the Dioxus UI to WASM into
`dist/`/`target/dx/...`), compiles the Rust core + plugin to a `libpalmannotate_lib.so`
for `aarch64-linux-android`, then Gradle assembles + R8-minifies + signs the APK.

**Output:**

```
src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

(~28 MiB, arm64-v8a only, debug-signed → installs directly.)

First build is slow (downloads crates + Gradle deps). A clean release build is the
heaviest step because `Cargo.toml` uses `lto = true` + `codegen-units = 1` +
`opt-level = "s"` for a small `.so`.

> **Disk:** `target/` grows to ~11 GB during a release build. Keep several GB free.
> If space is tight, `rm -rf target` between full builds.

---

## 4. Install & run on the device

USB debugging on, device connected (`adb devices` shows it):

```powershell
$adb = 'C:\tools\android-sdk\platform-tools\adb.exe'
$apk = 'src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release.apk'
& $adb install -r $apk
& $adb shell am force-stop dev.sawitulm.palmannotate.rust
& $adb shell am start -n dev.sawitulm.palmannotate.rust/.MainActivity
```

A running app keeps the OLD code until restarted — always force-stop + relaunch
after install. See `HANDOFF.md` §2 for logcat / WebView DevTools debugging.

---

## 5. Fast iteration (no full APK each time)

| Command | Use | Speed |
|---|---|---|
| `cargo check -p palmannotate-ui --target wasm32-unknown-unknown` | Catch UI (`src/`) compile errors | ~5–50 s |
| `cargo check -p palmannotate` | Catch backend (`src-tauri`) compile errors | ~20 s |
| `cargo test -p palmannotate-core` | Run core logic tests (32 tests) | <1 s |
| `cargo test` | Whole-workspace tests | seconds |
| `dx build` | Build the Dioxus WASM only (UI smoke) | ~15 s |
| `cargo fmt` / `cargo clippy --all-targets -- -D warnings` | Lint gate | — |

Always run the two `cargo check`s before a slow Android build — they catch almost
everything except native/Gradle issues.

---

## 6. Clean-clone smoke test (the "new machine" path)

```bash
git clone <repo> && cd PalmAnnotate-Rust
rustup target add wasm32-unknown-unknown aarch64-linux-android   # if not present
cargo test -p palmannotate-core                                  # logic OK?
cargo check -p palmannotate-ui --target wasm32-unknown-unknown   # UI compiles?
# set Android env vars (§3), then:
cargo tauri android build --apk --target aarch64                 # produce APK
# install (§4) and verify on device.
```

If `cargo tauri android build` complains about a missing NDK or SDK, the env vars
in §3 are wrong — fix those first (it is almost never a code problem).

---

## 7. Common pitfalls

- **App crashes instantly on launch (`NoSuchMethodError: MainActivity.getId()`):**
  R8 stripped JNI classes. `src-tauri/gen/android/app/proguard-rules.pro` must keep
  `dev.sawitulm.**`, `app.tauri.**`, `@Command` methods, and native methods. (Fixed
  in this repo — don't remove those rules.)
- **Camera preview black / "Camera ready":** the device camera uses WebView
  `getUserMedia`, not a native pump. It needs the `CAMERA` permission (in the
  manifest) and the `RustWebChromeClient` to grant it (it does). Tauri's
  `localhost` origin is a secure context, so getUserMedia is allowed.
- **Back gesture closes the app:** `MainActivity.kt` must bridge back to the
  in-app `window.__paBack()` (Tauri disables wry's default back handling).
- **`JAVA_HOME is not set` / Gradle exit 49:** set the env vars from §3.
- **Re-running `cargo tauri android init`** overwrites `MainActivity.kt` and
  `proguard-rules.pro` — re-apply them if you ever do.
