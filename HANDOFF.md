# HANDOFF

## 1. Project Structure

### Full File Tree With Purpose

- `.cursor/rules/after-changes.mdc` - Local editor/agent rule metadata.
- `.env.example` - Example environment variables for Python implementation.
- `.gitignore` - Git ignore patterns.
- `.python-version` - Python toolchain pin.
- `Justfile` - Task runner commands for Python workflow.
- `LICENSE` - MIT license text.
- `README.md` - High-level project readme (Python-era orientation).
- `TODO.md` - Legacy TODO list.
- `architecture.md` - Earlier architecture draft.
- `architecture-review.md` - Review findings and concerns.
- `architecture-v2.md` - Final architecture used for Swift rewrite decisions.
- `rewrite-analysis.md` - Analysis notes for rewrite strategy.
- `pyproject.toml` - Root Python packaging and dependencies.
- `uv.lock` - Python dependency lockfile.
- `misc/pyannote-diarization/batch.sh` - Batch diarization helper script.
- `misc/pyannote-diarization/diarization.py` - Optional diarization utility.
- `misc/pyannote-diarization/pyproject.toml` - Diarization tool Python config.
- `misc/pyannote-diarization/uv.lock` - Diarization tool lockfile.
- `src/quedo/__init__.py` - Python package metadata.
- `src/quedo/cli.py` - Python CLI entrypoint.
- `src/quedo/env.py` - Python config validation and parsing.
- `src/quedo/log_config.py` - Python logging setup.
- `src/quedo/main.py` - Python runtime orchestration.
- `src/quedo/paths.py` - Python path helpers.
- `src/quedo/permissions.py` - Python permission checks.
- `src/quedo/packages/__init__.py` - Python package namespace.
- `src/quedo/packages/audio_recorder/__init__.py` - Python recorder package marker.
- `src/quedo/packages/audio_recorder/main.py` - Python audio recording implementation.
- `src/quedo/packages/keyboard_listener/__init__.py` - Python keyboard package marker.
- `src/quedo/packages/keyboard_listener/main.py` - Python hotkey listener implementation.
- `src/quedo/packages/notifications/__init__.py` - Python notifications package marker.
- `src/quedo/packages/notifications/main.py` - Python notification/sound implementation.
- `src/quedo/packages/transcriber/__init__.py` - Python transcriber package marker.
- `src/quedo/packages/transcriber/main.py` - Python provider/chunking/transcription implementation.
- `macos/Package.swift` - Swift Package manifest for core, app, and CLI targets.
- `macos/Sources/Quedo/main.swift` - App entrypoint bootstrapping `NSApplication`.
- `macos/Sources/Quedo/AppDelegate.swift` - Service wiring, windows, Sparkle bootstrap, login-item registration.
- `macos/Sources/Quedo/AppController.swift` - Lifecycle coordinator actor connecting hotkey/audio/transcription/output/history.
- `macos/Sources/Quedo/MenuBarController.swift` - `NSStatusItem` icon/menu UI and action dispatch.
- `macos/Sources/Quedo/OnboardingCoordinator.swift` - First-run checks (hotkey presence, loopback check, provider connectivity).
- `macos/Sources/Quedo/PreferencesView.swift` - SwiftUI preferences UI + save/load VM.
- `macos/Sources/Quedo/HistoryView.swift` - SwiftUI session history view.
- `macos/Sources/QuedoCLI/main.swift` - CLI executable (`quedo-cli`) commands.
- `macos/Sources/QuedoCore/CoreTypes.swift` - Shared enums/data models/settings schemas.
- `macos/Sources/QuedoCore/StateMachine.swift` - Authoritative `AppPhase` state machine and UI contract mapping.
- `macos/Sources/QuedoCore/ConfigurationManager.swift` - UserDefaults + Keychain config/secrets + validation.
- `macos/Sources/QuedoCore/Logging.swift` - OSLog + rotating JSONL log mirror.
- `macos/Sources/QuedoCore/PermissionCoordinator.swift` - Mic/accessibility/input-monitoring checks and settings deep links.
- `macos/Sources/QuedoCore/AudioCaptureEngine.swift` - AVAudioEngine capture with retry and watchdog logic.
- `macos/Sources/QuedoCore/HotkeyManager.swift` - Carbon `RegisterEventHotKey` registration and callbacks.
- `macos/Sources/QuedoCore/TranscriptionPipeline.swift` - Provider orchestration, retries, fallback cooldown/reprobe, cleanup.
- `macos/Sources/QuedoCore/OutputRouter.swift` - Clipboard and synthetic Cmd+V output routing.
- `macos/Sources/QuedoCore/HistoryStore.swift` - SQLite persistence + legacy Python history migration.
- `macos/Sources/QuedoCore/DiagnosticsCenter.swift` - Structured events, metrics, recovery budget, diagnostics export.
- `macos/Sources/QuedoCore/Providers/TranscriptionProvider.swift` - Provider protocol + request/response/error contracts.
- `macos/Sources/QuedoCore/Providers/MultipartFormData.swift` - Multipart body builder utility.
- `macos/Sources/QuedoCore/Providers/GroqProvider.swift` - Groq HTTP transcription/health implementation.
- `macos/Sources/QuedoCore/Providers/OpenAIProvider.swift` - OpenAI HTTP transcription/health implementation.
- `macos/Tests/QuedoCoreTests/StateMachineTests.swift` - State transition and UI contract tests.
- `macos/Tests/QuedoCoreTests/ConfigurationTests.swift` - Config defaults/validation/save-load tests.
- `macos/Tests/QuedoCoreTests/HistoryStoreTests.swift` - SQLite write/list smoke test.
- `macos/Tests/QuedoCoreTests/TranscriptionPipelineTests.swift` - Pipeline fallback and cleanup tests.

### SPM Organization

`macos/Package.swift` defines:
- Products:
  - `QuedoCore` (library)
  - `Quedo` (executable app target)
  - `quedo-cli` (CLI executable)
- Targets:
  - `QuedoCore` (core logic, links `sqlite3`)
  - `Quedo` (AppKit/SwiftUI app, depends on core + Sparkle)
  - `QuedoCLI` (ArgumentParser CLI, depends on core)
  - `QuedoCoreTests`
- Dependencies:
  - `swift-argument-parser`
  - `Sparkle`

## 2. Architecture Overview

### High-Level Data Flow

1. Global hotkey event enters via `HotkeyManager` (Carbon).
2. `AppControllerActor` transitions lifecycle `ready -> arming -> recording`.
3. `AudioCaptureEngine` captures mic audio to a local file.
4. On stop, `TranscriptionPipeline` sends audio to provider:
   - Primary provider first.
   - Primary retry once after 1s for retryable errors.
   - Fallback provider attempt on failure.
5. Deterministic transcript cleanup runs.
6. `OutputRouter` routes to clipboard and optional synthetic paste.
7. `HistoryStore` persists session metadata/transcript/events.
8. `DiagnosticsCenter` records metrics/events/recovery attempts.

### Key Design Decisions

- Actors are used for stateful subsystems (`LifecycleStateMachine`, `HistoryStore`, `DiagnosticsCenter`, etc.) to isolate mutable state.
- A single lifecycle state model (`AppPhase`) is the source of truth.
- Providers are abstracted behind `TranscriptionProvider` for Groq/OpenAI parity and fallback.
- Persistence is SQLite + media file store, with explicit legacy migration.

### Threading / Concurrency Model

- Coordination uses Swift concurrency (`async/await`) and actor isolation.
- UI updates are funneled to main thread via callback marked `@MainActor`.
- Background retry/probe loops use `Task` in pipeline/diagnostics.

### Module Connections

- `AppDelegate` wires all services and constructs `AppControllerActor`.
- `MenuBarController` forwards menu actions to app controller.
- `AppControllerActor` orchestrates all core modules and lifecycle transitions.
- Core modules are independent and testable by protocol/data contracts.

## 3. What Is Implemented

### App Layer Files

- `macos/Sources/Quedo/main.swift`: Complete app process entrypoint.
- `macos/Sources/Quedo/AppDelegate.swift`: Largely complete bootstrap/wiring, including Sparkle controller init and login-item registration trigger.
- `macos/Sources/Quedo/AppController.swift`: Core workflow implemented (start/stop/retry/cancel, save history, transition phases). Completeness: medium; behavior exists but not validated on device.
- `macos/Sources/Quedo/MenuBarController.swift`: Deterministic menu rendering and action dispatch present. Completeness: medium-high.
- `macos/Sources/Quedo/OnboardingCoordinator.swift`: Reliability gates implemented in simplified form. Completeness: medium (logic exists but not full UX/remediation loops from spec).
- `macos/Sources/Quedo/PreferencesView.swift`: Preferences UI and validation save path implemented. Completeness: medium.
- `macos/Sources/Quedo/HistoryView.swift`: History list UI with refresh implemented. Completeness: medium.

### CLI File

- `macos/Sources/QuedoCLI/main.swift`: Commands implemented for `start`, `stop`, `status`, `logs`, `doctor`, `config show`, `history list/play/transcribe`, `transcribe <file>`. Completeness: medium-high for v1 parity subset.

### Core Files

- `macos/Sources/QuedoCore/CoreTypes.swift`: Complete shared type model and defaults.
- `macos/Sources/QuedoCore/StateMachine.swift`: Complete deterministic state machine + UI contract mapping.
- `macos/Sources/QuedoCore/ConfigurationManager.swift`: Complete settings load/save/validate + keychain helpers.
- `macos/Sources/QuedoCore/Logging.swift`: Complete OSLog + size-based rotating file logger.
- `macos/Sources/QuedoCore/PermissionCoordinator.swift`: Permission checks and settings deep links implemented.
- `macos/Sources/QuedoCore/AudioCaptureEngine.swift`: AVAudioEngine capture path and watchdog/retry skeleton implemented.
- `macos/Sources/QuedoCore/HotkeyManager.swift`: Carbon registration and callback dispatch implemented.
- `macos/Sources/QuedoCore/TranscriptionPipeline.swift`: Primary retry, fallback attempt, 30s sticky fallback and 60s reprobe implemented.
- `macos/Sources/QuedoCore/OutputRouter.swift`: Clipboard + synthetic paste implementation.
- `macos/Sources/QuedoCore/HistoryStore.swift`: SQLite schema, save/list/event persistence, legacy folder migration implemented.
- `macos/Sources/QuedoCore/DiagnosticsCenter.swift`: Event recording, metric tracking, recovery budget, diagnostics zip export implemented.
- `macos/Sources/QuedoCore/Providers/TranscriptionProvider.swift`: Complete protocol/contract file.
- `macos/Sources/QuedoCore/Providers/MultipartFormData.swift`: Utility complete.
- `macos/Sources/QuedoCore/Providers/GroqProvider.swift`: Provider HTTP path implemented.
- `macos/Sources/QuedoCore/Providers/OpenAIProvider.swift`: Provider HTTP path implemented.

### Tests

- `macos/Tests/QuedoCoreTests/StateMachineTests.swift`: Transition guards and UI contract assertions.
- `macos/Tests/QuedoCoreTests/ConfigurationTests.swift`: Defaults, validation aggregation, save/load.
- `macos/Tests/QuedoCoreTests/HistoryStoreTests.swift`: Session persistence smoke test.
- `macos/Tests/QuedoCoreTests/TranscriptionPipelineTests.swift`: Fallback behavior and cleanup rules.

### Assumptions/Shortcuts Due to Linux-Only Environment

- Could not compile or run the Swift package (no `swift` toolchain available locally).
- Could not run AppKit, AVFoundation, Carbon, permissions, or Sparkle behaviors on-device.
- No Xcode project, signing, entitlement verification, or notarization test was possible.
- Some AppKit/Carbon APIs were written to best effort and need immediate compile pass on Mac.

## 4. What Is NOT Implemented / Still Needs Work

### macOS Distribution/Packaging Gaps

- No Xcode `.xcodeproj` or `.xcworkspace` configured for app distribution.
- No `Info.plist` created for app bundle metadata/privacy strings.
- No `.entitlements` files created for Direct vs MAS profiles.
- No code-signing identities/team provisioning setup.
- No notarization or Sparkle appcast publishing pipeline.
- No app icon/asset catalog.

### architecture-v2.md Gaps Not Yet Completed

- UI state contract is partially mapped; not all state-specific sounds/copy/actions are wired end-to-end in UX.
- First-run gates do not yet implement full remediation loops and exact pass criteria timing instrumentation.
- Recovery budget enforcement is partially implemented but not fully integrated across all subsystem paths.
- Sound cues (`NSSound`) are not implemented.
- User notifications (`UNUserNotificationCenter`) are not implemented.
- Diagnostics SLO rollups/alerts/release-gate enforcement are not fully implemented.
- Reliability suite/soak/sleep-wake automation from Section 12 is not implemented.
- Contract schema files under `contracts/schema/*.json` are not present.
- Linux shared contract conformance test runners are not implemented.
- MAS-adaptation behavior/profile switching is incomplete (only high-level output mode handling exists).

### Placeholder / Needs Real Device Validation

- Audio capture watchdog behavior and stop-timeout logic need real-device verification.
- Synthetic Cmd+V event injection needs accessibility validation in foreground apps.
- Carbon hotkey registration/re-registration behavior across sleep/wake needs real test.
- Provider multipart payload compatibility for Groq/OpenAI should be validated against live API responses.
- SQLite media path behavior for replay/retranscribe should be validated with realistic session files.

### Sparkle Integration Status

- Dependency added in `Package.swift`.
- `SPUStandardUpdaterController` is instantiated for direct profile in `AppDelegate`.
- Missing: updater feed URL, signing keys, appcast generation, and release channel flow.

### SQLite / GRDB Wiring Status

- SQLite is wired directly using `sqlite3` C API in `HistoryStore.swift`.
- GRDB is not integrated.
- If GRDB is desired, replace manual SQL prep/bind code with GRDB models/migrations.

## 5. Build Instructions

### Prerequisites

- Apple Silicon Mac.
- macOS 14.0+.
- Xcode 16.x (Swift 6 toolchain).
- Command line tools installed (`xcode-select --install`).

### Build via SwiftPM (CLI)

From repo root:

```bash
cd macos
swift package resolve
swift build
swift test
```

### Build via Xcode

1. Open Xcode.
2. `File -> Open...` and select `macos/Package.swift`.
3. Let dependencies resolve.
4. Select target `Quedo` and build.
5. Select target `QuedoCLI` if testing CLI executable.

### SPM Dependencies

- `swift-argument-parser` resolves automatically from GitHub.
- `Sparkle` resolves automatically from GitHub.

### Expected Build Issues

- App-bundle/distribution setup is incomplete until `Info.plist`, entitlements, and signing are added.
- Some AppKit/Carbon API usage may need compile fixes on the first real Mac build pass.

## 6. Testing Instructions

### Run Tests

```bash
cd macos
swift test
```

### What Tests Currently Cover

- Lifecycle transitions and contract mapping.
- Settings validation and persistence.
- History save/list smoke behavior.
- Transcription pipeline fallback and cleanup.

### What Tests Do Not Cover

- AppKit menu bar behavior.
- AVAudioEngine live recording behavior.
- Carbon hotkey runtime behavior.
- Permissions prompts and settings deep links.
- Sparkle updater flow.
- Login item registration behavior.

### Manual Real-Mac Testing Required

- Microphone capture quality and watchdog behavior.
- Global hotkey start/stop/cancel/retry.
- Accessibility + synthetic paste at cursor.
- Provider connectivity and fallback handling.
- Sleep/wake and device route-change recovery.

## 7. Xcode Project Setup

### Opening Strategy

- Open `macos/Package.swift` directly in Xcode for initial compile/test.
- For shipping, create/maintain an Xcode app project that links the package targets and supports signing/notarization.

### Signing/Entitlements Needed

Direct profile:
- App Sandbox: off.
- Network client: on.
- Audio input: on.
- Accessibility and (optional compatibility mode) Input Monitoring granted by user.

MAS profile:
- App Sandbox: on.
- Network client: on.
- Audio input: on.
- Clipboard-only output (no synthetic paste).

### Hardened Runtime / Sandbox

- Direct distribution: Hardened runtime enabled for notarization.
- MAS: sandbox required with entitlement-limited behavior.

### Info.plist Keys Required

At minimum:
- `NSMicrophoneUsageDescription`
- `LSUIElement` (menu bar app behavior)
- App bundle metadata (`CFBundleIdentifier`, version/build)
- Any Sparkle-related keys as required by your release pipeline.

## 8. First Launch Checklist

1. Launch app and confirm menu bar icon appears.
2. Confirm onboarding path starts when setup is incomplete.
3. Enter at least one valid API key (Groq or OpenAI).
4. Grant microphone permission.
5. Grant accessibility permission if using paste-at-cursor.
6. Verify hotkey triggers arming/recording/stop.
7. Verify 2-second mic check works and audio file is created.
8. Verify provider connectivity check passes for at least one provider.
9. Run one full session and confirm clipboard/paste output.
10. Open history and verify session persisted.
11. Run `quedo-cli doctor` and `quedo-cli status` for operational sanity checks.

## 9. Key Files to Read First

1. `architecture-v2.md`
2. `macos/Package.swift`
3. `macos/Sources/QuedoCore/StateMachine.swift`
4. `macos/Sources/Quedo/AppController.swift`
5. `macos/Sources/QuedoCore/AudioCaptureEngine.swift`
6. `macos/Sources/QuedoCore/TranscriptionPipeline.swift`
7. `macos/Sources/QuedoCore/HistoryStore.swift`
8. `macos/Sources/Quedo/AppDelegate.swift`
9. `macos/Sources/Quedo/MenuBarController.swift`
10. `macos/Sources/QuedoCLI/main.swift`

## 10. Reference Documents

- `architecture-v2.md` - Finalized implementation architecture and reliability requirements (authoritative source).
- `architecture-review.md` - Reviewer findings that motivated architecture hardening.
- `rewrite-analysis.md` - Rewrite planning notes and tradeoff analysis.
- `architecture.md` - Earlier architecture baseline for historical comparison.
