# PalmAnnotate-Rust — Direction & Roadmap

Where this app is going, why, and how we know when each stage is done. This is the
"north star" doc. For *current* state see [STATUS.md](STATUS.md); for the detailed
parity feature matrix see [PARITY_PLAN.md](PARITY_PLAN.md).

---

## 1. Mission

PalmAnnotate is an **offline Android field tool** for oil-palm fresh-fruit-bunch
(FFB) work: capture multi-view photos of a tree, annotate bunches (classes B1–B4),
deduplicate the same bunch seen across adjacent sides, count unique bunches, and
export YOLO + schema-v4 JSON datasets — all on-device, no network.

This repository is the **Rust + Tauri 2 + Dioxus rewrite** of the proven
`../PalmAnnotate-Android` (Capacitor + vanilla JS) app.

## 2. North star

> **Become a drop-in behavioral replacement for `PalmAnnotate-Android`, then move
> past it on size, speed, and maintainability — without losing any field behavior.**

The UI may look different (simple, Dioxus-native) but the **operator workflow,
persisted data, filenames, class/adjacency rules, exports, and device behavior must
match** the JS app. The JS app is the read-only behavioral reference; the Rust app
is the future.

### Why Rust/Tauri instead of staying on JS/Capacitor
- One typed Rust core (`palmannotate-core`) for model, storage, dedup, results,
  schema v4, quality, YOLO — unit-testable and platform-independent.
- Smaller, faster native binary; Kotlin only where it must be (CameraX, Orbbec,
  Storage Access Framework).
- Stronger guarantees: atomic writes, capture rollback, conflict-safe import.

## 3. Phases

### Phase 0 — Reach parity (CURRENT)
Make every field workflow behave like the JS app and ship a clean APK. This is the
only committed phase. Done when the **Completion Gate** (§4) is fully green.

Progress so far (see CHANGELOG/STATUS):
- ✅ Core data contract (schema v4, YOLO, dedup, counts) — tested.
- ✅ App launches on device (R8/JNI crash fixed).
- ✅ Device camera live preview via getUserMedia (verified on device).
- 🟡 Full capture→review→annotate→dedup→results→export flow — implemented, needs an
  end-to-end on-device pass.
- 🟡 Back navigation, SAF mirror, GPS, annotlog, delete cascade — implemented,
  pending device confirmation.
- ⛔ Orbbec RGB-D — needs the Gemini 335L on a powered USB hub.

### Phase 1 — Harden for the field (after parity)
Only once parity is proven:
- Orbbec lifecycle robustness: USB attach/detach, PD charge-through host-role drops,
  RGB/depth sync, ordered teardown (the JS app's hard-won behavior).
- CameraX/getUserMedia edge cases: permission deny→grant, stop/reopen, low light.
- Resume/restart correctness across full process death; SAF folder re-adoption on a
  fresh install from the folder alone.
- Battery/thermal during long capture sessions on the tablet.

### Phase 2 — Beyond the JS app (candidates, NOT committed)
Ideas the architecture already enables; to be confirmed with the operator before any
work starts — listed so the direction is visible, not as promises:
- **Desktop build** for review/QA: the platform-independent core + Dioxus UI can run
  on desktop for annotating/reviewing datasets off the tablet.
- **Detector improvements**: newer/quantized ONNX model, faster on-device inference.
- **Size/perf**: further `.so` and asset shrinking, lazy model load, preview cost.
- **Dataset tooling**: batch export, dataset stats, inter-annotator agreement from
  the annotlog (suggestions-vs-final) sidecars.

## 4. Completion Gate (definition of "drop-in replacement")

Parity is reached when **every row in [STATUS.md](STATUS.md) is ✅** and the on-device
acceptance checklist passes on the Xiaomi Pad 6:

1. SAF export folder picked + persisted; New Session blocked without it.
2. 4-side and 8-side capture complete (getUserMedia / Orbbec).
3. Captured images render in Review and Annotate (not black).
4. SAF mirror writes dataset/metadata/Output into the chosen folder (no error).
5. Detector runs → boxes enter UNASSIGNED → assign B1–B4 → remove false boxes.
6. annotlog sidecar written (suggestions vs final).
7. Dedup adjacent links incl. wrap-around persist.
8. Compute unique bunches → export JSON v4 + YOLO (+CSV + identity), locally + SAF.
9. Delete tree/session removes ALL artifacts incl. SAF mirror + annotlog.
10. GPS permission prompt + coords stored.
11. Survives full app restart (sessions/trees reload).
12. Back gesture navigates in-app and exits only from Home.
13. Orbbec preview/capture + detach/reconnect recovery (needs hardware).

Until all of the above hold, the honest status is **"not yet a drop-in replacement."**

## 5. Principles (don't drift from these)

- Action-first, visually compact UI; no product claims, decorative status text, or
  architecture labels. Don't expose a page/button until it does real work.
- Preserve the JS storage contract: schema v4 fields, filenames, class/adjacency
  rules, delete behavior, resumability.
- Local app data is the reliable working store; the SAF folder is a best-effort
  public mirror that never rolls back primary data.
- State what is **verified on device** vs assumed. A green `cargo test` is not
  on-device proof.
- Keep the install/runtime footprint small enough for practical field deployment.
