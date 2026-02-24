# quedo macOS Rewrite Architecture (Wave 2)

## 1. Scope and Quality Bar

This document defines the full product and system architecture for a **world-class macOS transcription assistant** that competes with top-tier tools.

Constraints and priorities for this design:

- Final quality over build speed.
- Reliability and graceful recovery over minimal code size.
- macOS is primary; Linux is secondary.
- Architecture only; no implementation code.

Primary user promise:

- press shortcut
- speak
- receive correct text quickly
- app never feels broken, stuck, or confusing

## 2. Product Requirements

## 2.1 Reliability SLOs (Production Targets)

- Hotkey detection success rate: >= 99.95% per session.
- Recording start latency (shortcut to first captured frame): p95 <= 120 ms.
- Recording stop-to-first-text latency (network provider): p95 <= 2.5 s for <= 20 s clips.
- Crash-free sessions: >= 99.9%.
- Self-recovery from audio route change/sleep wake without restart: >= 99.5%.
- Zero silent failures: every failure produces user-visible status and actionable recovery path.

## 2.2 UX Principles

- Always show clear state.
- One primary action per state.
- Never block user on hidden background work.
- Recover automatically first; ask user second.
- Use sound + visual cues consistently and optionally.

## 3. Complete Feature Inventory

## 3.1 Current Python Feature Inventory (No Drops)

This section inventories all features present in the current codebase. The Swift product must preserve these capabilities (directly or with improved UX).

### 3.1.1 CLI and daemon lifecycle

From `src/quedo/cli.py`:

- Version command output with upgrade hint.
- `init` wizard:
  - collect API key
  - configure three hotkeys (toggle, retry, cancel)
  - set transcription language (`auto` or explicit language)
  - set output mode (`clipboard`, `paste_on_cursor`, both, or none)
  - set whisper model
  - set API timeout
  - set vocabulary hints
  - write `config.env` with `0600` permissions
  - validate configuration
  - run permission checks (Accessibility, Input Monitoring, Microphone)
  - optionally open settings panes and re-check
  - optional daemon start
- Daemon commands:
  - `start`
  - `stop`
  - `restart`
  - `status`
- PID tracking in state dir and stale PID cleanup.
- Startup crash detection via stderr log check.

### 3.1.2 Diagnostics and support commands

- `doctor` command:
  - permission status
  - config validation
  - daemon running status
  - input device query
  - quick mic probe
  - issue summary with fix hints
  - `--fix` to open settings panes
- `logs` command:
  - tail recent lines
  - choose info log or stderr log

### 3.1.3 Config and history

- `config show`
- `config edit` with editor selection and validation loop.
- If daemon running, restart after config edit.
- History recording persistence in dated folders.
- `history list`
- `history transcribe N`
- `history play N`

### 3.1.4 Recording, transcription, output runtime

From `src/quedo/main.py` and package modules:

- Global hotkeys: toggle record, cancel record, retry last transcription.
- Recording lock prevents overlapping recording flows.
- Start/stop/cancel recording.
- Progress sounds and optional progress notifications.
- Transcription of in-memory audio arrays via Groq Whisper API.
- Clipboard output and optional paste-at-cursor via synthetic Cmd+V.
- Background save to history: audio FLAC + transcription text.
- Warn user if suspiciously silent recording (mic permissions or route issue).
- Warn user if paste likely failed due Accessibility permission.

### 3.1.5 Audio recorder reliability workarounds

From `src/quedo/packages/audio_recorder/main.py`:

- PortAudio prewarm via device query.
- Stream open retries with escalating recovery.
- Soft and aggressive backend reset paths.
- Sample-rate fallback strategy.
- Compact diagnostics logging for device state on failures.
- Stream close watchdog timeout to avoid hangs.
- Failure classification (`stream_open_error`, `empty_audio`, `cancelled`).
- Consecutive open failure counters.

### 3.1.6 Transcriber capabilities

From `src/quedo/packages/transcriber/main.py`:

- In-memory array transcription.
- File transcription for audio and video inputs.
- ffmpeg extraction/transcoding for unsupported formats.
- Long-audio chunking and rolling prompt context.
- Known hallucination cleanup pass.
- Request-too-large handling with actionable error.
- Configurable model/timeout/language/vocabulary prompt.

### 3.1.7 Permissions and platform behavior

From `src/quedo/permissions.py`:

- Accessibility trust check.
- Input Monitoring preflight check.
- Mic probe check.
- Open targeted System Settings panes for each permission.

### 3.1.8 Notifications and audio cues

From `src/quedo/packages/notifications/main.py`:

- Info/error desktop notifications.
- Sound cues for key stages and failures.

### 3.1.9 Paths and logging

From `src/quedo/paths.py` and `log_config.py`:

- XDG-style directories for config/data/state.
- Rotating debug/info logs.
- stderr startup log.

### 3.1.10 Misc tool in repository

From `misc/pyannote-diarization/`:

- standalone diarization pipeline tool (not daemon core), with chunking and transcript merge.

## 3.2 New Features Required for World-Class Product

### 3.2.1 Product shell and discoverability

- Menu bar app with explicit state iconography.
- First-run setup wizard with guided permission walkthrough.
- Rich Preferences window with search and categories.
- In-app command palette (open settings, start recording, retry, export logs).
- Onboarding checklist with completion status.

### 3.2.2 Recording UX

- Instant visual recording indicator in menu bar.
- Optional floating HUD near cursor:
  - recording timer
  - input level meter/waveform
  - cancel hint
- Voice activity indicator and silence warning while recording.
- Configurable recording controls:
  - toggle mode
  - hold-to-talk mode
  - optional auto-stop on silence

### 3.2.3 Transcription UX

- Clear phases:
  - recording
  - processing audio
  - transcribing
  - output complete
- Optional partial result streaming when provider supports it.
- Retry with same audio and with alternate provider/model.

### 3.2.4 Output modes

- Clipboard only.
- Paste at cursor.
- Clipboard + paste.
- Append to active note/file target.
- Save as draft in history only.
- Optional post-processing transforms:
  - punctuation mode
  - capitalization mode
  - custom text replacements

### 3.2.5 History and session management

- Searchable history browser (text + metadata filters).
- Replay recordings.
- Re-transcribe with different provider/model/language.
- Export recording + transcript bundle.
- Pin/favorite entries.

### 3.2.6 Provider architecture

- Multiple providers:
  - Groq Whisper
  - OpenAI transcription endpoint
  - Local provider (WhisperKit or equivalent)
- Per-provider health checks.
- Provider fallback policy and quality profiles.

### 3.2.7 Reliability and recovery

- Explicit lifecycle manager for sleep/wake, route changes, and device hot-swap.
- Automatic capture engine rearm without app restart.
- Persistent crash-safe journal of in-flight recording state.
- Circuit breaker for repeated provider failures.

### 3.2.8 Supportability

- Structured diagnostics events.
- One-click "Export Diagnostics" bundle.
- Optional anonymized telemetry toggles.
- Self-check page similar to `doctor` but UI-native.

### 3.2.9 Accessibility and inclusion

- Full VoiceOver labels and announcements.
- Keyboard navigation for all UI.
- Reduced motion mode.
- High contrast icon mode.

### 3.2.10 Distribution and updates

- Direct distribution channel with in-app updater.
- Mac App Store channel variant.
- Automatic migration from legacy Python config/history.

## 4. System Architecture

## 4.1 Process Model

Use a two-layer macOS architecture for isolation and resilience:

- `Quedo.app` (UI + menu bar + onboarding + settings)
- `WhisperCoreService` (core orchestration, transcription pipeline, history persistence)

Communication model:

- XPC interface with typed request/response contracts.

Rationale:

- UI stays responsive even if core pipeline restarts.
- Core can be independently supervised and restarted.
- Fault containment for audio/provider failures.

## 4.2 High-Level Module Map

### 4.2.1 UI App modules

- `MenuBarController`
  - owns status item/menu and icon state rendering
- `OnboardingCoordinator`
  - first-run flow orchestration
- `PreferencesCoordinator`
  - settings windows, validation, persistence hooks
- `HUDController`
  - optional floating recording/transcription overlay
- `DiagnosticsUI`
  - health checks, logs, export
- `CoreClientXPC`
  - client gateway to core service

### 4.2.2 Core service modules

- `AppLifecycleOrchestrator` (actor)
  - global state machine coordinator
- `HotkeyManager` (actor)
  - registration, conflict checks, callbacks
- `AudioCaptureEngine` (actor)
  - device lifecycle, frame stream, route handling
- `TranscriptionPipeline` (actor)
  - request preparation, provider dispatch, retries, fallback
- `ProviderRegistry` (actor)
  - provider adapters, health and capability matrix
- `OutputRouter` (actor)
  - clipboard/paste/file outputs and post-processing
- `HistoryStore` (actor)
  - metadata DB + media file storage
- `PermissionCoordinator` (actor)
  - permission checks and guidance states
- `DiagnosticsCenter` (actor)
  - structured logs, traces, export bundle
- `MigrationManager`
  - import from Python config/history formats

## 4.3 Public Interfaces and Protocols

Define stable protocol contracts to prevent module coupling.

### 4.3.1 Core protocols

- `HotkeyService`
  - register/unregister/update shortcut profiles
  - emit shortcut events
- `AudioCaptureService`
  - arm/start/stop/cancel capture session
  - publish capture metrics and frame stream
- `TranscriptionService`
  - transcribe(audio, context, provider policy)
  - stream progress and partials
- `ProviderAdapter`
  - capability descriptors (streaming, max payload, languages)
  - health check
  - transcribe request execution
- `OutputService`
  - route transcript to selected sinks
- `PermissionService`
  - current permission state
  - required action list
- `HistoryRepository`
  - CRUD/search/export for sessions
- `DiagnosticsService`
  - log event
  - emit support bundle

### 4.3.2 Shared event model

All components publish typed events to an internal event bus:

- `AppStateChanged`
- `PermissionStateChanged`
- `HotkeyTriggered`
- `CaptureStarted/CaptureLevel/CaptureStopped/CaptureFailed`
- `TranscriptionStarted/Partial/Completed/Failed`
- `OutputApplied/OutputFailed`
- `RecoveryAttempted/RecoverySucceeded/RecoveryFailed`

## 4.4 Storage Architecture

### 4.4.1 Settings

- `UserDefaults` for non-sensitive preferences.
- Keychain for API keys/tokens.
- Versioned settings schema with migration IDs.

### 4.4.2 History

- SQLite metadata store (indexable, searchable).
- Filesystem media store for audio attachments.
- Optional transcript revision history table.

Metadata per session:

- session ID
- timestamps (start/end)
- provider/model/language
- capture device route
- duration and audio stats
- transcript text
- output targets applied
- failure/recovery events

### 4.4.3 Diagnostics

- Unified logging via `OSLog` with subsystem/categories.
- Rolling file mirror for support export.
- Crash reports + last 200 significant domain events snapshot.

## 5. State Machines

## 5.1 Global App Lifecycle State Machine

States:

1. `coldStart`
2. `migratingLegacyData`
3. `onboardingRequired`
4. `permissionSetup`
5. `ready`
6. `degraded` (feature available but with missing permissions/provider)
7. `recovering` (automatic recovery in progress)
8. `fatal` (unrecoverable startup issue)
9. `shuttingDown`

Key transitions:

- `coldStart -> migratingLegacyData` when legacy artifacts found.
- `coldStart -> onboardingRequired` if first run.
- `onboardingRequired -> permissionSetup` after config complete.
- `permissionSetup -> ready` when minimum required permissions satisfied.
- `ready -> degraded` when provider/auth/permission becomes unavailable.
- `ready -> recovering` on route/sleep/hotkey backend disruptions.
- `recovering -> ready` when self-heal succeeds.
- `recovering -> degraded` when recovery exhausted.

Recovery policy:

- bounded retries with exponential backoff
- event-level reason tracking
- user notification only when auto-recovery fails or degrades quality

## 5.2 Recording Session State Machine

States:

1. `idle`
2. `arming` (validating permissions + engine readiness)
3. `recording`
4. `stopping`
5. `encoding`
6. `transcribing`
7. `outputting`
8. `completed`
9. `cancelled`
10. `failed`

Transition rules:

- Shortcut while `idle` triggers `arming`.
- Successful engine start moves to `recording`.
- Stop action moves to `stopping`, then `encoding`.
- `encoding -> transcribing -> outputting -> completed`.
- Cancel from `recording` or `transcribing` moves to `cancelled` with cleanup.
- Any unrecoverable exception routes to `failed` with user-visible error.

Guardrails:

- single active session globally
- explicit cancellation propagation through tasks
- deterministic cleanup on every terminal state

## 5.3 Permission State Machine

Per-permission state:

- `unknown`
- `granted`
- `denied`
- `restricted`
- `notApplicable`

Permissions tracked:

- microphone
- accessibility (for paste automation and advanced integrations)
- input monitoring (only if required by selected hotkey backend)
- notifications (optional but recommended)

Composite policy states:

- `minimumReady` (record + transcript + clipboard)
- `enhancedReady` (paste automation enabled)
- `blocked` (cannot capture audio)

## 5.4 Provider Health State Machine

Per provider:

- `unknown`
- `healthy`
- `degraded`
- `unavailable`
- `rateLimited`
- `authInvalid`

Fallback strategy:

- primary provider attempt
- one retry if transient network
- switch to next policy-approved provider
- if all fail, persist draft audio and present one-click retry options

## 6. Concurrency and Threading Model

Use Swift structured concurrency with actor isolation.

## 6.1 Actor boundaries

Actors:

- `AppLifecycleOrchestrator`
- `AudioCaptureEngine`
- `HotkeyManager`
- `TranscriptionPipeline`
- `ProviderRegistry`
- `OutputRouter`
- `HistoryStore`
- `DiagnosticsCenter`

MainActor usage:

- menu bar state/UI updates
- onboarding and preferences interactions

## 6.2 Task topology

Per recording session:

- Task A: capture stream + level metrics
- Task B: optional partial encoding/chunk preparation
- Task C: provider transcription task(s)
- Task D: output application task
- Task E: persistence task (history + diagnostics)

Cancellation semantics:

- session cancel token cascades to all tasks
- cleanup task always runs (defer semantics at orchestration layer)

## 6.3 Backpressure and memory control

- bounded frame ring buffer
- spill-to-temp-file for long sessions
- maximum in-memory PCM budget
- provider chunk dispatch capped by concurrency policy

## 7. Data Flow Diagrams

## 7.1 End-to-end recording flow

```text
Global Shortcut
  -> HotkeyManager
  -> AppLifecycleOrchestrator.startSession()
  -> AudioCaptureEngine (mic frames)
  -> Encoder (FLAC/WAV as provider requires)
  -> TranscriptionPipeline
  -> ProviderAdapter (Groq/OpenAI/Local)
  -> Transcript result stream
  -> OutputRouter (clipboard/paste/file)
  -> HistoryStore (audio + transcript + metadata)
  -> MenuBarController/HUD update
```

## 7.2 Recovery flow for route change/sleep wake

```text
System Notification (sleep/wake/route/device change)
  -> AppLifecycleOrchestrator
  -> AudioCaptureEngine.reconfigure()
  -> hotkey and provider health checks
  -> if success: state=ready
  -> if failure: state=degraded + guided user action
```

## 7.3 Permission-guided output flow

```text
Output request: paste_at_cursor
  -> PermissionCoordinator checks Accessibility
  -> granted: perform paste automation
  -> denied: fallback clipboard + show guided permission card
```

## 8. UX Design Specification

## 8.1 Menu Bar Icon States

Define deterministic icon system with optional monochrome mode.

States and meanings:

1. `idle`
- Icon: neutral microphone outline
- Tooltip: "Ready"

2. `arming`
- Icon: pulse ring
- Tooltip: "Preparing microphone..."

3. `recording`
- Icon: solid red dot + timer badge
- Tooltip: "Recording (shortcut to stop)"

4. `processing`
- Icon: waveform spinner
- Tooltip: "Processing audio..."

5. `transcribing`
- Icon: text waveform animation
- Tooltip: "Transcribing..."

6. `outputting`
- Icon: clipboard/paste glyph pulse
- Tooltip: "Delivering text..."

7. `success`
- Icon: check pulse (brief)
- Tooltip: "Done"

8. `degraded`
- Icon: amber warning badge
- Tooltip: "Limited functionality"

9. `error`
- Icon: red exclamation badge
- Tooltip: short actionable summary

## 8.2 Menu Drop-down Information Architecture

Primary section:

- Start/Stop Recording
- Retry Last Transcription
- Open History

Secondary section:

- Output Mode quick toggle
- Active language/provider summary
- Device status summary

Support section:

- Permissions status
- Diagnostics and Export Logs
- Preferences
- Quit

Rules:

- Never show disabled item without reason text.
- Always include direct fix action when an item is disabled.

## 8.3 Recording Flow UX

### Start

- User presses hotkey.
- Immediate haptic/sound cue (configurable).
- Icon moves to `arming` then `recording`.
- Optional HUD appears near cursor with timer + level meter.

### During recording

- Level meter continuously updates.
- Silence hint after configurable period (for accidental mute route).
- Cancel action clearly shown (`Esc` or configured shortcut).

### Stop

- User presses hotkey again.
- UI shifts to `processing` then `transcribing`.
- Progress text shown in menu tooltip and optional HUD.

### Finish

- Success sound.
- Confirmation toast:
  - where transcript was sent
  - one-click "Undo Paste" if applicable

## 8.4 Error UX and Recovery UX

Error classes and surface strategy:

1. User-actionable permissions errors
- Surface: inline permission card + menu warning badge.
- CTA: "Open Settings" deep link.

2. Transient technical errors (network timeout)
- Surface: brief non-modal banner.
- CTA: "Retry now".

3. Persistent backend errors (provider auth invalid)
- Surface: sticky degraded state + settings shortcut.
- CTA: "Fix API key".

4. Internal system errors
- Surface: error panel with incident ID.
- CTA: "Export diagnostics".

No dead ends:

- every error view includes
  - what failed
  - what still works
  - next action

## 8.5 Keyboard Shortcut Discoverability

- First-run step explicitly captures shortcuts.
- Preferences has dedicated Shortcuts page with conflict warnings.
- Menu shows current primary shortcut.
- Command palette includes "Reveal Shortcuts".
- On failed shortcut registration: real-time conflict diagnosis.

## 8.6 First-Run Wizard (Step-by-Step)

Step 1: Welcome and promise

- Explain value and privacy model (cloud/local providers).

Step 2: Provider setup

- Choose provider(s).
- Enter API keys (stored in keychain).
- Validate with live test request.

Step 3: Language and output defaults

- Default language or auto.
- Output mode selection.
- Optional vocabulary hints.

Step 4: Shortcut assignment

- Capture primary/secondary shortcuts.
- Detect conflicts with system and app shortcuts.

Step 5: Permissions guided flow

- Microphone: explain why, request, verify.
- Accessibility (if paste mode enabled): explain why, deep-link, verify.
- Input monitoring if backend requires it: explain and verify.

Step 6: Live test

- 5-second test recording.
- show transcript and chosen output path.

Step 7: Completion

- Show quick cheat sheet.
- Offer launch at login toggle.

## 8.7 Preferences Organization

Sections:

1. `General`
- launch at login
- menu bar icon style
- sound cues and notifications

2. `Shortcuts`
- all command shortcuts and conflict view

3. `Recording`
- input device selection
- mode (toggle/hold)
- silence auto-stop thresholds
- quality/format knobs

4. `Transcription`
- provider priority list
- model/language defaults
- vocabulary hints
- streaming partials toggle

5. `Output`
- clipboard/paste/file targets
- text post-processing rules

6. `History`
- retention policy
- storage location
- export/import

7. `Diagnostics`
- self-check
- log level
- export diagnostics

8. `Advanced`
- fallback policies
- experimental providers/features

## 9. Platform Integration Details

## 9.1 macOS Frameworks and APIs by Capability

- Menu bar: `AppKit` (`NSStatusItem`, status menu handling).
- UI: `SwiftUI` + AppKit bridge where needed.
- Audio capture: `AVFoundation` (`AVAudioEngine`, input node tap).
- Device and route change observation: CoreAudio/AVFAudio notifications + workspace wake notifications.
- Hotkeys:
  - primary: Carbon `RegisterEventHotKey` pathway for robust global shortcuts
  - optional advanced backend: event tap for richer behavior where distribution allows
- Clipboard: `NSPasteboard`.
- Paste automation: Accessibility + event synthesis path.
- Permissions:
  - microphone via AVFoundation usage prompt
  - accessibility trust checks via ApplicationServices
  - input monitoring preflight checks via CoreGraphics APIs
- Notifications: `UserNotifications`.
- Keychain: `Security` framework.
- Logging: `OSLog` and signposts.
- Launch at login: `ServiceManagement` (`SMAppService`).
- Storage: SQLite (`GRDB` or Core Data) + filesystem.

## 9.2 Entitlements and Privacy Declarations

Define two shipping profiles.

### 9.2.1 Mac App Store profile

Entitlements:

- App Sandbox enabled.
- Network client entitlement.
- Audio input entitlement.
- Any required temporary exceptions only if absolutely necessary and review-compliant.

Info.plist privacy usage strings:

- `NSMicrophoneUsageDescription`
- any additional usage descriptions required by final selected APIs

Update model:

- no third-party updater in MAS build (updates via App Store).

### 9.2.2 Direct distribution profile

Entitlements/capabilities:

- Hardened Runtime enabled.
- Required capabilities for selected integration paths.
- Accessibility-related integration enabled for paste automation and advanced controls.

Info.plist privacy strings:

- microphone usage rationale
- optional accessibility/automation rationale keys where applicable

Update model:

- Sparkle channel for signed delta/full updates.

## 9.3 Sandbox Considerations

- Keep core architecture sandbox-compatible where feasible.
- Feature flags for distribution-dependent behaviors.

Examples:

- If a behavior is limited under MAS policy, degrade to clipboard-only mode and surface rationale.
- Keep shortcut backend pluggable so MAS-safe backend is default.

## 9.4 Code Signing, Notarization, and Release Pipeline

### 9.4.1 Direct build pipeline

1. `xcodebuild archive`
2. Developer ID signing
3. Hardened runtime validation
4. Notarization submission (`notarytool`)
5. Staple notarization ticket
6. Publish Sparkle appcast + signed artifacts

Release gates:

- entitlement audit passes
- privacy strings present
- smoke tests on clean VM

### 9.4.2 Mac App Store pipeline

1. archive and sign for App Store
2. App Store Connect upload
3. TestFlight canary
4. phased production release

## 9.5 Minimum macOS Version Target

Recommended target: **macOS 14+**.

Rationale:

- modern Swift concurrency and SwiftUI/AppKit interop stability
- current security/permission behavior consistency
- reduced matrix improves reliability and QA depth

Validation matrix:

- current major
- previous major
- beta of next major

## 9.6 Auto-update Framework Choice

Recommendation:

- Direct channel: Sparkle 2.
- MAS channel: App Store updates only.

Rationale:

- Sparkle is mature, signed update ecosystem, good rollback behavior.
- clean split avoids MAS policy conflicts.

## 10. Error Handling and Recovery Strategy

## 10.1 Error Taxonomy

1. `PermissionError`
2. `DeviceUnavailableError`
3. `CaptureEngineError`
4. `ProviderAuthError`
5. `ProviderRateLimitError`
6. `ProviderTransientNetworkError`
7. `OutputDeliveryError`
8. `PersistenceError`
9. `InternalInvariantError`

Each error carries:

- user-facing message
- actionable next step
- severity
- retry eligibility
- diagnostics payload

## 10.2 Recovery Policies

- Transient provider errors: bounded automatic retries.
- Route/device changes: silent rearm attempts before user notification.
- Output failures:
  - paste failure falls back to clipboard
  - keep transcript in quick access buffer
- On persistent failure:
  - app enters `degraded`
  - provides guided fix panel

## 10.3 User-visible Recovery UX

- non-modal warnings for transient issues
- persistent badges for blocking issues
- one-click fix actions where possible
- explicit "what still works" messaging

## 11. Testing Strategy

## 11.1 Test Pyramid

- Unit tests: pure logic and state machines.
- Component tests: actor modules with mocks/fakes.
- Integration tests: end-to-end pipeline with deterministic audio fixtures.
- UI tests: onboarding, settings, menu states, error surfaces.
- Reliability tests: sleep/wake, route churn, repeated long sessions.

## 11.2 Testing Audio Capture Without Real Mic

Approach:

- abstract capture input source interface.
- test provider feeds deterministic PCM fixtures from files.
- optional synthetic generator source (tone/speech-like envelopes).
- integration on CI with virtual loopback devices where available.

Assertions:

- start latency budgets
- no dropped frames beyond threshold
- deterministic stop behavior
- proper handling of silence and clipping

## 11.3 Hotkey Testing Strategy

- Unit: shortcut parsing/validation/conflict detection.
- Component: backend mock verifies register/unregister correctness.
- Integration: instrumented host app sends synthetic key events where permitted.
- Manual matrix for distribution profiles and permission conditions.

## 11.4 UI Testing Strategy

- XCUITest with accessibility identifiers for all controls.
- Snapshot tests for menu icon and key settings states.
- Onboarding scenario tests:
  - happy path
  - denied permissions
  - invalid API key
- Error path UI tests for every major error taxonomy class.

## 11.5 Transcription Pipeline Integration Testing

- Mock provider server for deterministic responses and injected failures.
- Golden transcript fixtures for normalization/post-processing logic.
- Failure injection:
  - timeout
  - 401 auth
  - 413 payload too large
  - 429 rate limit
  - malformed response
- Verify fallback ordering and user surface behavior.

## 11.6 Reliability and Soak Testing

- 8-hour and 24-hour daemon endurance tests.
- Repeated sleep/wake cycles.
- Device churn tests:
  - USB mic unplug/replug
  - Bluetooth headset connect/disconnect
  - dock/undock transitions
- Memory and file descriptor leak checks.
- Crash recovery and state consistency checks.

## 11.7 Release Exit Criteria

Release blocked unless all pass:

- no P0/P1 open bugs
- SLO thresholds met in staging matrix
- onboarding completion success >= target
- diagnostics export verified on clean system

## 12. Security and Privacy Architecture

- API keys only in keychain.
- Never log raw secrets.
- Transcript logging off by default in analytics streams.
- Explicit consent for telemetry and crash upload.
- Data retention settings with hard-delete behavior.
- Export bundle redaction options (default redacted).

## 13. Legacy Migration Plan (Python -> Swift)

## 13.1 Data to migrate

- config values from `config.env`
- history directory structure and transcript files
- optional carry-over of last used hotkeys/provider

## 13.2 Migration phases

1. Detect legacy install on first launch.
2. Preview import summary to user.
3. Import settings into new schema.
4. Rebind secrets to keychain.
5. Index legacy history into SQLite metadata store.
6. Keep original files untouched unless user opts cleanup.

## 13.3 Compatibility guarantees

- no destructive overwrite of existing data
- reversible migration markers
- user can re-run import from diagnostics tools

## 14. Linux Companion Notes (Rust)

Linux remains secondary, but architecture mirrors macOS concepts.

## 14.1 Rust companion module mapping

- `lifecycle_orchestrator`
- `hotkey_manager`
- `audio_capture`
- `transcription_pipeline`
- `provider_registry`
- `output_router`
- `history_store`
- `diagnostics`

## 14.2 Platform-specific differences

- Hotkeys:
  - X11 first-class support
  - Wayland behavior documented with fallbacks
- Clipboard/paste integration varies by desktop environment.
- Tray UX differs across DEs; keep command model consistent.

## 14.3 Shared contracts between macOS and Linux

Use shared schema package (JSON schema or protobuf) for:

- settings model
- session metadata
- event types
- provider request/response envelope
- diagnostics bundle manifest

Benefits:

- common test fixtures
- cross-platform parity checks
- easier support tooling

## 15. Implementation Readiness Checklist

This architecture is implementation-ready when these artifacts are created from it:

- module ownership map
- protocol contract document
- state-machine transition table with exhaustive events
- UX copy and icon asset spec
- entitlements matrix per distribution profile
- CI release pipelines (direct + MAS)
- test matrix and fixture catalog

At that point, engineering can execute without unresolved product or architecture questions.

## 16. Detailed Component Specifications

This section defines each component at the level required for implementation handoff.

## 16.1 `AppLifecycleOrchestrator` (Core, actor)

Responsibilities:

- Own global lifecycle state.
- Coordinate startup sequencing and dependency readiness.
- Trigger and supervise auto-recovery workflows.

Owned state:

- `appState`
- `activeSessionID`
- `degradedReasons[]`
- `recoveryBudget` counters by subsystem

Public operations:

- `bootstrap()`
- `handleSystemEvent(event)`
- `requestStartSession(trigger)`
- `requestStopSession(reason)`
- `requestRetryLastSession()`
- `shutdown()`

Input events:

- app launch
- sleep/wake notifications
- audio route/device changes
- provider health changes
- permission changes
- hotkey triggers

Output events:

- lifecycle changes
- user-facing state summaries
- diagnostics checkpoints

Error handling:

- converts subsystem failures into typed lifecycle degradations
- enforces max retries before entering `degraded`

## 16.2 `HotkeyManager` (Core, actor)

Responsibilities:

- Register global shortcut set.
- Detect shortcut conflicts and apply backend policy.
- Emit normalized hotkey events.

Backends:

- Backend A (default): Carbon global hotkeys (`RegisterEventHotKey`).
- Backend B (optional): event tap for advanced control when allowed.

Owned state:

- active bindings map
- backend in use
- registration health status

Public operations:

- `setBindings(profile)`
- `validateBinding(binding)`
- `activate()`
- `deactivate()`
- `reloadBackend(policy)`

Errors:

- registration conflict
- backend unavailable
- permission blocked (if advanced backend selected)

Recovery:

- auto-fallback from Backend B to Backend A
- deferred re-registration on wake/login session changes

## 16.3 `AudioCaptureEngine` (Core, actor)

Responsibilities:

- Configure `AVAudioEngine`.
- Start/stop/cancel capture sessions.
- Maintain route/device awareness and resilient reconfiguration.

Owned state:

- capture state machine state
- selected input device and route metadata
- level meter rolling stats
- active frame buffer references

Public operations:

- `prepareSession(config)`
- `startCapture(sessionID)`
- `stopCapture(sessionID)`
- `cancelCapture(sessionID)`
- `reconfigureForRouteChange(change)`
- `probeMicrophoneHealth()`

Events emitted:

- capture started/stopped/cancelled
- level updates
- silent capture warnings
- route change diagnostics

Errors:

- permission denied
- engine start failure
- route unavailable
- buffer/encoder overflow

Recovery:

- reinitialize audio engine on route change
- fallback input route selection by priority list
- bounded restart attempts before degrading

## 16.4 `TranscriptionPipeline` (Core, actor)

Responsibilities:

- Convert captured audio to provider requests.
- Execute provider policy (primary + fallback).
- Merge partial/final results and normalization steps.

Owned state:

- active transcription tasks by session
- provider policy and fallback chain
- chunking strategy state

Public operations:

- `transcribe(sessionAudio, context)`
- `retry(sessionID, strategy)`
- `cancel(sessionID)`

Pipeline stages:

1. format normalization
2. chunk planning
3. provider dispatch
4. partial aggregation
5. final normalization
6. hallucination/text-rule cleanup

Errors:

- auth invalid
- rate limited
- payload too large
- timeout/network
- malformed provider response

Recovery:

- transient retry
- automatic provider fallback
- reduce chunk size and retry on payload errors
- persist unresolved session for manual retry

## 16.5 `ProviderRegistry` and provider adapters

Responsibilities:

- Keep provider catalog and capabilities.
- Route requests to adapter by policy.
- Track provider health state over time.

Adapters:

- `GroqProviderAdapter`
- `OpenAIProviderAdapter`
- `LocalWhisperProviderAdapter`

Required adapter contract:

- capabilities (streaming, limits, languages)
- auth validator
- health check probe
- transcribe (streaming/non-streaming)

Policy attributes:

- preferred provider
- allowed fallback set
- privacy mode constraints (cloud-only/local-only/hybrid)

## 16.6 `OutputRouter` (Core, actor)

Responsibilities:

- Route transcript to one or more output targets.
- Ensure fallback behavior when a target fails.

Targets:

- clipboard
- paste at cursor
- append to file
- draft only (history)

Public operations:

- `applyOutputs(transcript, targets)`
- `undoLastPasteIfSupported(sessionID)`

Failure policy:

- paste failure -> clipboard fallback
- append failure -> draft save + error surface

Permissions:

- checks Accessibility before paste automation

## 16.7 `HistoryStore` (Core, actor)

Responsibilities:

- Persist session metadata + transcript + audio pointers.
- Serve search and replay queries.

Public operations:

- `saveSession(record)`
- `updateTranscriptRevision(sessionID, revision)`
- `search(query)`
- `getSession(sessionID)`
- `exportBundle(sessionID)`
- `deleteSession(sessionID)`

Data retention policies:

- keep forever
- rolling window (days)
- max storage budget

## 16.8 `PermissionCoordinator` (Core, actor)

Responsibilities:

- Evaluate effective permission posture.
- Emit actionable remediation guidance.

Public operations:

- `refreshPermissionState()`
- `requiredActionsFor(featureSet)`
- `openSystemSettings(pane)`

Feature gating examples:

- no microphone -> block capture
- no Accessibility -> allow clipboard-only mode, disable paste mode

## 16.9 `DiagnosticsCenter` (Core, actor)

Responsibilities:

- ingest typed events and logs
- maintain incident timeline per session
- build support bundles

Public operations:

- `record(event)`
- `startIncident(tag)`
- `closeIncident(id)`
- `exportSupportBundle(options)`

Bundle contents:

- app/core versions
- config snapshot (redacted)
- recent lifecycle/capture/provider events
- crash markers
- recent logs

## 16.10 `MenuBarController` (UI)

Responsibilities:

- render icon states and menus
- display stateful actions and badges
- connect user actions to core client

Public actions:

- start/stop recording
- retry
- open history/preferences/diagnostics
- quick output/language/provider toggles

## 16.11 `OnboardingCoordinator` (UI)

Responsibilities:

- run first-run flow with checkpoints
- persist completion progress
- validate provider/auth/hotkeys/permissions in-line

Completion criteria:

- provider configured
- primary shortcut valid
- microphone verified by test clip
- at least one output mode configured

## 16.12 `PreferencesCoordinator` (UI)

Responsibilities:

- host category pages
- validate and apply settings changes
- stage changes requiring restart and perform graceful apply flow

## 16.13 `CoreClientXPC` (UI bridge)

Responsibilities:

- typed request/response API
- stream lifecycle/session events to UI
- handle core reconnection on restart

Failure behavior:

- if core unavailable, UI shows degraded state and auto-reconnects with backoff

## 17. Distribution Profiles and Entitlements Matrix

## 17.1 Common capabilities (both channels)

- microphone capture
- network client for cloud providers
- launch at login
- keychain secret storage

## 17.2 Entitlements by channel

### 17.2.1 Mac App Store profile

Required entitlement keys:

- `com.apple.security.app-sandbox`
- `com.apple.security.network.client`
- `com.apple.security.device.audio-input`

Optional entitlement keys (only if needed and review-approved):

- `com.apple.security.automation.apple-events`

Policy notes:

- avoid any behavior that resembles keylogging.
- keep hotkeys to MAS-compliant backend path.
- if paste automation path is restricted, enforce clipboard-first mode and explain in UI.

### 17.2.2 Direct distribution profile

Required security posture:

- Hardened Runtime enabled in signing settings.
- Notarization required for release artifacts.

Entitlements typically enabled:

- `com.apple.security.network.client`
- `com.apple.security.device.audio-input`
- additional automation/accessibility-related capabilities only where needed

Operational note:

- final entitlement set is validated with automated audit in CI and runtime smoke tests.

## 17.3 Privacy strings checklist

Mandatory:

- `NSMicrophoneUsageDescription`

Conditional (based on enabled integration paths):

- automation/accessibility rationale strings for APIs requiring user disclosure

## 18. Configuration Schema (Versioned)

All settings are versioned and migrated with explicit schema revisions.

## 18.1 Core settings keys

- `settings.version`
- `general.launchAtLogin`
- `general.showHUD`
- `general.playSounds`
- `general.showNotifications`
- `shortcuts.toggleRecording`
- `shortcuts.cancelRecording`
- `shortcuts.retryTranscription`
- `recording.mode` (toggle/hold)
- `recording.autoStopSilence.enabled`
- `recording.autoStopSilence.thresholdSeconds`
- `recording.preferredInputDeviceID`
- `transcription.defaultLanguage`
- `transcription.vocabularyHints[]`
- `transcription.providerPriority[]`
- `transcription.defaultModelByProvider{}`
- `transcription.timeoutSeconds`
- `transcription.enableStreamingPartials`
- `output.targets[]`
- `output.appendFilePath`
- `output.postProcessingRules[]`
- `history.retentionPolicy`
- `history.maxStorageMB`
- `diagnostics.logLevel`
- `diagnostics.telemetryEnabled`

## 18.2 Secret material

Stored only in Keychain:

- `provider.groq.apiKey`
- `provider.openai.apiKey`
- local-model provider secrets if needed

## 18.3 Runtime-derived settings

Computed values, not directly user-editable:

- effective provider fallback chain
- effective permission posture
- degraded reason set

## 19. Failure Mode and Recovery Matrix

## 19.1 Capture and device failures

1. Mic permission denied
- Detection: capture start preflight fails.
- User surface: blocking permission card.
- Auto recovery: none until permission granted.
- Manual action: open mic settings.

2. Input route disappears mid-session
- Detection: route change callback with missing route.
- User surface: transient warning + fallback route notice.
- Auto recovery: rebind to default available input.
- Manual action: choose device in Preferences if fallback undesired.

3. Sleep/wake capture engine invalidated
- Detection: first capture frame timeout after wake.
- User surface: subtle recovering indicator.
- Auto recovery: engine teardown/recreate and rearm.
- Manual action: only if rearm fails repeatedly.

4. Bluetooth headset flaps
- Detection: repeated route add/remove events.
- User surface: debounced warning after N flaps.
- Auto recovery: debounce route-switch, keep stable route.
- Manual action: lock preferred device.

## 19.2 Provider failures

1. API key invalid
- Detection: auth response (401/403 class).
- User surface: persistent degraded badge + "Fix API key" CTA.
- Auto recovery: none.

2. Rate limited
- Detection: provider rate-limit response.
- User surface: transient warning with retry ETA.
- Auto recovery: exponential backoff; fallback provider if allowed.

3. Payload too large
- Detection: request too large response.
- User surface: non-blocking message; automatic chunking note.
- Auto recovery: reduce chunk size and retry.

4. Provider timeout
- Detection: timeout budget exceeded.
- User surface: toast with retry option.
- Auto recovery: one retry + optional fallback provider.

## 19.3 Output failures

1. Paste automation blocked
- Detection: Accessibility check fails or injection error.
- User surface: warning with "Clipboard copy completed" confirmation.
- Auto recovery: clipboard fallback.

2. Clipboard write failure
- Detection: pasteboard operation error.
- User surface: blocking error with transcript preview and copy button.
- Auto recovery: retry write once.

3. Append-to-file path invalid
- Detection: file open/write error.
- User surface: error with path chooser CTA.
- Auto recovery: store transcript in history draft.

## 20. UI Copy and Interaction Contracts

## 20.1 Mandatory microcopy contracts

Every state message follows:

- plain-language status
- next expected step
- optional action shortcut

Examples:

- Recording: "Listening. Press ⌃⇧1 to stop."
- Processing: "Preparing transcript..."
- Degraded no Accessibility: "Pasting is unavailable. Text is copied to clipboard."

## 20.2 Notification policy

- Success notifications are subtle and optional.
- Failure notifications are actionable, never vague.
- Repeated identical failures are coalesced (cooldown).

## 20.3 Sound policy

- Start/stop/success/error sounds independently configurable.
- Respect Focus mode where applicable.
- Sound disabled automatically during screen sharing mode if user enables privacy mode.

## 21. Observability and Telemetry Model

## 21.1 Local observability

Capture per session:

- latency milestones (start, stop, transcript received, output applied)
- capture stats (input route, level profile)
- provider stats (attempt count, fallback used)
- output stats (targets, success/failure)

## 21.2 Optional telemetry events (user opt-in)

- anonymized event counters
- latency percentiles
- error code frequencies

Never send:

- raw transcript text
- raw audio
- API keys or device serials

## 22. Compatibility Surface for Existing Power Users

Even with a GUI-first app, preserve power-user workflows.

Recommended companion CLI (thin shim to core service):

- `quedo status`
- `quedo doctor`
- `quedo logs`
- `quedo transcribe <file>`
- `quedo history list|play|transcribe`

CLI behavior:

- calls XPC/core APIs instead of owning separate logic
- ensures no feature divergence between GUI and CLI

## 23. Open Decisions (Must Be Resolved Before Build Freeze)

1. Local provider default for offline mode (WhisperKit baseline vs alternate).
2. MAS channel behavior if full paste automation is constrained.
3. Minimum supported Intel vs Apple Silicon policy.
4. Whether hold-to-talk is default or optional advanced mode.
5. Final transcript post-processing rule engine complexity level.

These are product policy decisions, not architecture blockers.
