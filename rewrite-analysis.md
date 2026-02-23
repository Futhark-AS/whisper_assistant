# Rewrite Recommendation for `whisper_assistant`

## Executive Summary

**Top recommendation (winner): Swift-first native architecture for macOS, with a small Rust companion for Linux.**

If forced to name one primary stack: **Swift + AppKit/AVFoundation on macOS**.

Reason: your failures are concentrated in exactly the two places where native macOS frameworks are strongest and Python wrappers are weakest for long-running desktop daemons:

- microphone stream lifecycle stability
- global hotkey/event tap stability and permissions

A pure cross-platform stack (Rust-only, Go-only, or Tauri) can reduce some Python pain, but none beats Swift on macOS reliability + UX with fewer recovery hacks.

## What the Current Python Code Tells Us

Your business logic is simple, but runtime glue is fragile:

- `src/whisper_assistant/packages/audio_recorder/main.py` has extensive defensive logic for PortAudio instability: retry ladders, fallback sample rates, aggressive backend resets (`_terminate/_initialize`), diagnostics, close timeouts, and forced recovery.
- `src/whisper_assistant/main.py` adds another layer of auto-retry around recorder startup failures.
- `src/whisper_assistant/packages/keyboard_listener/main.py` is a thin `pynput` wrapper with limited self-healing when listeners jam.
- `src/whisper_assistant/permissions.py` uses best-effort/fail-open checks, which reduces crash risk but can hide broken permission states.

In short: much of your complexity is not product complexity. It is runtime compensation for abstraction leaks.

## Market / Tooling Signals from Existing macOS Voice Apps

Public docs for MacWhisper/Whisper Transcription emphasize local processing and deep platform integration (dictation into any field, App Store constraints, local WhisperKit engine, local speaker recognition). These are the same capability areas where native integration matters most.

- MacWhisper/Whisper Transcription docs show local-first + dictation + App Store constraint differences.
- Superwhisper docs highlight cross-platform availability and native desktop behavior, but do not publicly detail implementation stack.

Inference: best-in-class macOS voice tools optimize around native platform behavior first, then extend outward.

## Option-by-Option Evaluation

## 1) Swift Native (AppKit + AVFoundation/AVAudioEngine)

### Reliability

- Best fit for macOS audio lifecycle and permission model.
- No PortAudio wrapper layer; direct use of Apple APIs.

### Simplicity

- Fewer resilience workarounds than current Python approach.
- Straight path for menu bar app, login item, keychain, pasteboard, and permissions.

### Install / UX

- Best user experience on macOS (signed app bundle, proper prompts, login item, smooth updates).

### Linux

- Weak as a single-stack desktop answer. Swift runtime is fine on Linux, but macOS UI/framework parity is not.

**Verdict:** best primary stack for your stated priority (macOS reliability + UX).

## 2) Rust Native (cpal + desktop/event crates)

### Reliability

- Better than Python in many cases, but still depends on cross-platform abstraction layers.
- `cpal` is active and mature enough for production, but macOS edge bugs still exist and show up in issue history.

### Simplicity

- More engineering surface than Swift for polished macOS UX and permissions.

### Install / UX

- Good packaging potential, but native macOS polish usually takes more effort than Swift/AppKit.

### Linux

- Strong cross-platform story; easier than Swift for Linux companion.

**Verdict:** strong second place, best if single-language cross-platform were top priority.

## 3) Go + macOS APIs via cgo

### Reliability

- Can work, but real macOS integration often means cgo bridges and Obj-C/C glue.

### Simplicity

- Loses Go simplicity quickly when deep macOS desktop integration is required.

### Install / UX

- Practical, but more custom integration work for polished native behaviors.

### Linux

- Good language portability; desktop integration still mixed.

**Verdict:** viable but not optimal for this product shape.

## 4) Tauri (Rust backend + webview UI)

### Reliability

- Good for many desktop apps, but this app is systems-heavy (audio/hotkeys/permissions), not UI-heavy.
- You still end up writing non-trivial platform-specific Rust for macOS details.

### Simplicity

- Adds frontend/runtime layers that don’t buy much for a menu bar daemon.

### Install / UX

- Bundle tooling is solid, but you still own macOS entitlement + permission complexity.

### Linux

- Good cross-platform packaging, but some tray/hotkey behavior differences still matter in practice.

**Verdict:** overbuilt for your needs; wrong center of gravity.

## 5) Novel / Scripted approaches (Hammerspoon, Shortcuts, etc.)

### Reliability

- Great for power-user automation, weak foundation for a dependable commercial-grade daemon.

### Simplicity

- Fast prototyping, but long-term maintainability and distribution quality suffer.

**Verdict:** not a rewrite target.

## 6) Hybrid (Swift macOS app + Rust Linux companion)

This is the recommended architecture pattern.

- macOS gets maximum reliability and best UX through Swift-native stack.
- Linux gets a smaller Rust companion where Rust’s portability helps.
- Shared protocol/contracts keep business logic aligned without forcing lowest-common-denominator platform choices.

## Decision Matrix (1-5)

| Option | Reliability (macOS) | Simplicity | Install/UX (macOS) | Linux support | Total |
|---|---:|---:|---:|---:|---:|
| Swift native | 5 | 5 | 5 | 2 | 17 |
| Rust native | 4 | 3 | 3 | 4 | 14 |
| Go + cgo | 3 | 2 | 3 | 4 | 12 |
| Tauri | 3 | 2 | 3 | 4 | 12 |
| Hybrid (Swift + Rust Linux) | 5 | 4 | 5 | 4 | **18** |

## Recommended Target Architecture

## macOS (primary): Swift app (menu bar agent)

### Core modules

- `HotkeyService`
  - Primary: `RegisterEventHotKey` path (sandbox/App Store friendly behavior)
  - Optional advanced mode: CGEventTap for specific overrides where needed
- `RecordingEngine`
  - `AVAudioEngine` + input node tap
  - explicit state machine (`idle -> recording -> processing -> error`)
  - device route-change handling
- `TranscriptionClient`
  - `URLSession` client for Groq Whisper API
  - strict timeout/retry policy (small, explicit)
- `OutputService`
  - pasteboard write + Cmd+V injection
- `PermissionCoordinator`
  - mic/accessibility/input-monitoring guidance and status
- `HistoryStore`
  - same folder semantics as current app (or SQLite if desired)

### macOS platform integrations

- Login at startup: `SMAppService`
- Auto-updates: Sparkle
- Secrets: Keychain
- Packaging/signing/notarization from day one

## Linux (secondary): Rust companion daemon/CLI

- Minimal feature parity target first:
  - push-to-talk + record + Groq transcribe + clipboard output
- Keep global hotkey implementation conservative:
  - X11 support first
  - explicit caveat for Wayland environments
- No attempt to mirror full macOS UX initially

## Shared contract

- Common config schema and event model (JSON/protobuf)
- Shared transcription request/response format
- Shared history file layout where practical

## Migration Plan from Python

1. **Freeze behavior and capture baseline telemetry**
   - record startup crashes, hotkey misses, recording failures, and resume-from-sleep behavior.
2. **Build Swift vertical slice**
   - menu bar app + single hotkey + record/stop + Groq call + paste.
3. **Replace fragile paths first**
   - retire PortAudio/sounddevice path entirely on macOS.
4. **Add production hardening**
   - permission UX, login item, history, retries, structured logs.
5. **Pilot rollout on macOS**
   - dogfood and targeted external testers, measure restart frequency and missed hotkeys.
6. **Build Linux Rust companion**
   - ship as separate artifact with explicitly secondary support posture.
7. **Deprecate Python daemon**
   - retain legacy CLI command compatibility where helpful.

## Complexity Comparison vs Current Python

Current Python complexity is inflated by reliability scaffolding:

- `audio_recorder` and daemon orchestration contain layered recovery mechanisms that exist mostly to compensate for backend instability.
- Rewrite to Swift-native macOS stack should remove most of these compensations and replace them with smaller lifecycle logic around native APIs.

Expected outcome:

- less defensive code
- fewer daemon restarts
- fewer post-sleep failure modes
- clearer permission behavior

## Final Recommendation

Build the rewrite as a **Swift-native macOS product** and treat Linux as a **secondary Rust companion target**.

If you choose only one stack despite Linux: choose **Swift**. It maximizes what you care about most: reliability, simplicity, and a world-class macOS UX.

## Sources

- Repo code reviewed in full (all modules under `src/` + `misc/`).
- Apple CoreGraphics event tap docs: https://developer.apple.com/documentation/coregraphics/cgevent/tapcreate%28tap%3Aplace%3Aoptions%3Aeventsofinterest%3Acallback%3Auserinfo%3A%29
- Apple AVAudioEngine docs: https://developer.apple.com/documentation/avfaudio/avaudioengine
- Apple launchd guidance: https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html
- Apple SMAppService login items: https://developer.apple.com/documentation/servicemanagement/smappservice/loginitem%28identifier%3A%29
- PortAudio hotplug status: https://github.com/PortAudio/portaudio/wiki/HotPlug
- CPAL crate docs: https://docs.rs/crate/cpal/latest
- CPAL macOS issue example: https://github.com/RustAudio/cpal/issues/329
- Rust global hotkey crate docs (Linux X11 note): https://docs.rs/global-hotkey/latest/global_hotkey/
- Go cgo docs: https://pkg.go.dev/cmd/cgo
- Go hotkey package docs (`x/hotkey`): https://pkg.go.dev/golang.design/x/hotkey
- RobotGo requirements (GCC, macOS permissions): https://github.com/go-vgo/robotgo
- Tauri system tray docs (Linux unsupported tray event note): https://v2.tauri.app/learn/system-tray/
- Tauri global shortcut plugin docs: https://tauri.app/plugin/global-shortcut/
- Tauri macOS bundle/entitlements docs: https://v2.tauri.app/distribute/macos-application-bundle/
- Tauri microphone issue example: https://github.com/tauri-apps/tauri/issues/9928
- KeyboardShortcuts (Swift, sandbox/MAS compatible): https://github.com/sindresorhus/KeyboardShortcuts
- Hammerspoon hotkey/eventtap docs: https://www.hammerspoon.org/docs/hs.hotkey.html and https://www.hammerspoon.org/docs/hs.eventtap
- MacWhisper privacy/local processing: https://macwhisper.helpscoutdocs.com/article/52-keeping-transcriptions-private
- MacWhisper vs Whisper Transcription differences: https://macwhisper.helpscoutdocs.com/article/40-macwhisper-whisper-transcription-difference
- MacWhisper WhisperKit model notes: https://macwhisper.helpscoutdocs.com/article/29-switching-to-a-whisperkit-model
- MacWhisper local speaker recognition via WhisperKit engine: https://macwhisper.helpscoutdocs.com/article/32-automatic-speaker-recognition-in-macwhisper
- Superwhisper product docs: https://superwhisper.com/docs/get-started/introduction
