# quedo Architecture v2 (Final, Implementation-Ready)

## 1. Purpose

This is the final architecture for the Swift rewrite. It resolves all previously open decisions, preserves Python parity where required, simplifies state management for v1, and defines measurable reliability and release gates.

Primary goal:

- world-class, production-grade macOS transcription assistant with predictable behavior under sleep/wake, device churn, permission changes, and provider outages.

## 2. Frozen Product Decisions (Resolved)

## 2.1 Distribution strategy (primary vs secondary)

Decision:

- **Primary channel for v1: direct distribution (signed + notarized) with Sparkle updates.**
- Secondary channel: Mac App Store (MAS) build starts after direct v1 stability gate is met.

Rationale:

- Direct channel allows full paste-at-cursor behavior, faster hotfix release cadence, and lower review friction for reliability iteration.
- MAS remains important, but v1 architecture is optimized for reliability first, then policy-constrained packaging.

## 2.2 Hotkey backend by profile

Decision:

- **Both profiles use Carbon `RegisterEventHotKey` as primary backend.**
- Direct profile may optionally enable CGEventTap compatibility mode behind explicit setting and Input Monitoring grant.
- MAS profile does **not** use CGEventTap.

Fallback rules:

1. Register with Carbon.
2. If registration fails due key conflict: prompt remap flow immediately.
3. If Carbon fails unexpectedly at runtime:
- one automatic re-register attempt after 300 ms
- if still failing: enter degraded state with inline fix actions.
4. Direct profile only: user can opt into CGEventTap compatibility mode.

Entitlements / permission implications:

- Carbon path: no Input Monitoring entitlement requirement.
- CGEventTap path: requires Input Monitoring user permission; disabled by default.

## 2.3 Offline/local provider for v1

Decision:

- **No local/offline provider in v1.**
- v1 providers: Groq (primary), OpenAI (fallback).
- Local provider adapter interface exists but is not shipped in v1 runtime.

Rationale:

- Improves reliability focus and keeps v1 support surface contained.
- Avoids bundling model lifecycle complexity during rewrite stabilization.

## 2.4 Hardware baseline

Decision:

- **Apple Silicon only for v1.**
- Minimum OS: **macOS 14.0+**.
- Intel is intentionally unsupported in v1.

Rationale:

- Reduces audio/perf variance and test matrix size.
- Enables tighter reliability guarantees with one architecture target.

## 2.5 Recording interaction default

Decision:

- **Default mode: toggle-to-record (press once start, press once stop).**
- Hold-to-talk is supported in v1 as optional mode in Preferences.

Rationale:

- Preserves Python behavior and expected user muscle memory.

## 2.6 Post-processing scope for v1

Decision:

- v1 post-processing is deterministic and rule-based only:
  - trim/space normalization
  - known hallucination pattern removal
  - user-defined replacement dictionary
- No generative rewrite mode in v1.

Rationale:

- Keeps output predictable and debuggable.

## 2.7 Storage architecture (locked)

Decision:

- **SQLite metadata + media file store** (not dated-folder-only semantics).
- Python folder artifacts are migrated, indexed, and preserved.

Rationale:

- fast history search/filtering
- explicit schema migrations
- robust parity for replay/transcribe while supporting legacy files

## 3. V1 Scope and Non-Goals

V1 includes:

- global hotkey recording
- provider transcription with fallback (Groq -> OpenAI)
- clipboard and paste-at-cursor outputs (direct build)
- searchable history with replay + re-transcribe
- diagnostics UI + export bundle
- companion CLI parity subset
- onboarding with reliability checks (hotkey, mic loopback, provider test)

V1 non-goals:

- offline local model runtime
- multi-provider ranking/advanced orchestration
- Intel support

## 4. Simplified Runtime Architecture (Single Process)

To remove v1 over-engineering, architecture is single-process menu bar app with isolated services, not split UI/core daemons.

Process:

- `Quedo.app` hosts menu bar UI and all services.

Service modules:

- `AppController` (single lifecycle owner)
- `HotkeyService`
- `AudioService`
- `TranscriptionService`
- `OutputService`
- `HistoryService`
- `PermissionService`
- `DiagnosticsService`
- `SettingsService`
- `OnboardingService`

Concurrency model:

- Swift concurrency with one coordinator actor (`AppControllerActor`), plus dedicated actors for audio/transcription/history/diagnostics.
- UI updates on `MainActor` only.

## 5. Primary Lifecycle Model (Single State Machine)

There is one authoritative lifecycle state: `AppPhase`.

`AppPhase` values:

1. `booting`
2. `onboarding`
3. `ready`
4. `arming`
5. `recording`
6. `processing`
7. `streamingPartial`
8. `providerFallback`
9. `outputting`
10. `retryAvailable`
11. `degraded`
12. `shuttingDown`

Derived state (not separate machines):

- `degradedReason`: `permissions | noInputDevice | providerUnavailable | hotkeyFailure | internalError`
- `currentSessionID: UUID?`
- `fallbackAttempted: Bool`
- `lastErrorCode: String?`

Transition rules (deterministic):

- `booting -> onboarding` if first run or setup incomplete.
- `booting -> ready` if setup complete and minimum checks pass.
- `ready -> arming` on toggle/hold hotkey trigger.
- `arming -> recording` when first audio frame arrives.
- `arming -> degraded` on capture open failure after bounded retries.
- `recording -> processing` on stop action.
- `processing -> streamingPartial` if provider supports partials.
- `processing -> outputting` if non-streaming provider.
- `streamingPartial -> outputting` on final transcript.
- `processing/streamingPartial -> providerFallback` when primary fails and fallback eligible.
- `providerFallback -> outputting` on fallback success.
- `providerFallback -> retryAvailable` on fallback failure.
- `outputting -> ready` on output success.
- `outputting -> retryAvailable` on output failure with transcript preserved.
- `retryAvailable -> processing` when retry action selected.
- Any phase -> `degraded` on blocking runtime condition.
- `degraded -> ready` when blocking condition resolved and checks pass.

Guardrails:

- only one active session at a time.
- no hidden background state transitions without an emitted diagnostics event.

## 6. Deterministic UI State Contract

All states specify icon, notification copy, sound cue, and available actions.

| `AppPhase` | Menu Bar Icon | Notification Copy | Sound Cue | Actions Enabled |
|---|---|---|---|---|
| `ready` | gray mic | none | none | `Start Recording`, `Preferences`, `History`, `Run Checks` |
| `arming` | pulsing yellow mic | "Preparing microphone..." | `Hero` | `Cancel` |
| `recording` | solid red dot + timer | "Recording. Press shortcut to stop." | start cue already played | `Stop`, `Cancel`, `Mute Sound Cues` |
| `processing` | blue waveform spinner | "Processing audio..." | `Morse` | `Cancel` |
| `streamingPartial` | blue waveform + `...` badge | "Transcribing (live)..." | none | `Cancel`, `Copy Partial` |
| `providerFallback` | amber cloud swap icon | "Primary provider unavailable. Trying fallback..." | none | `Cancel`, `Force Retry Primary` (disabled until fallback completes) |
| `outputting` | green clipboard/paste glyph pulse | "Sending transcript..." | none | none |
| `retryAvailable` | amber retry arrow | "Could not complete. Retry is available." | `Basso` | `Retry`, `Change Provider`, `View Diagnostics` |
| `degraded` (`permissions`) | amber shield | "Permission needed for full functionality." | none | `Open Settings`, `Run Checks`, `Use Clipboard Only` |
| `degraded` (`noInputDevice`) | amber mic slash | "No input device detected." | `Basso` | `Refresh Devices`, `Select Device`, `Run Checks` |
| `degraded` (`providerUnavailable`) | amber cloud slash | "Transcription provider unavailable." | `Basso` | `Retry`, `Switch Provider`, `Open Provider Settings` |
| `degraded` (`hotkeyFailure`) | amber key icon | "Hotkey registration failed." | `Basso` | `Rebind Hotkey`, `Retry Registration` |
| `shuttingDown` | dim gray dot | none | none | `Quit` only |

Menu behavior under failure:

- In any `degraded` state, menu includes `Force Stop Recording` if a session is active.
- In any non-`ready` state, menu includes `View Last Error` and `Export Diagnostics`.

## 7. First-Run as Reliability Gate (Mandatory Checks)

Ready state is blocked until three reliability tests pass.

## 7.1 Hotkey verification test

Flow:

1. User sets primary hotkey.
2. App enters test mode for 20 seconds.
3. User must trigger hotkey successfully **2 consecutive times**.
4. Success criteria: both triggers create `arming` event within 200 ms of key event.

Remediation loop:

- If failed, show detected conflicts and prompt remap.
- Option to switch to safe default `Control+Shift+1`.

## 7.2 Microphone loopback check

Flow:

1. Record 2.0 seconds from selected input device.
2. Playback captured audio.
3. User confirms: `Heard audio clearly` / `Did not hear audio`.
4. App computes signal floor and peak.

Pass criteria:

- signal peak > configured threshold and user confirms playback.

Remediation loop:

- input device picker
- re-run test
- open macOS microphone settings

## 7.3 Provider connectivity test

Flow:

1. send 2-second synthetic sample to primary provider.
2. require response within 6 seconds.
3. run same test for fallback provider.

Pass criteria:

- primary succeeds OR fallback succeeds (at least one provider viable)

Remediation loop:

- API key edit
- timeout setting adjustment
- retry connectivity test

## 7.4 Onboarding completion rule

`ready` requires all:

- hotkey test passed
- mic loopback passed
- at least one provider connectivity test passed
- at least one output mode configured

## 8. Platform Integration and Profiles

## 8.1 Framework mapping

- Menu bar and app shell: AppKit (`NSStatusItem`) + SwiftUI settings windows.
- Audio capture: AVFoundation (`AVAudioEngine`).
- Global shortcuts: Carbon `RegisterEventHotKey`.
- Clipboard: `NSPasteboard`.
- Paste automation (direct build): Accessibility + synthetic key event path.
- Permissions checks: ApplicationServices/CoreGraphics preflight APIs where available.
- Notifications: UserNotifications.
- Secrets: Keychain Services.
- Logging: `OSLog` + rolling file mirror.
- Auto-update (direct): Sparkle 2.
- Launch at login: `SMAppService`.

## 8.2 Entitlements / capability matrix

| Capability | Direct Build | MAS Build |
|---|---|---|
| Network client | required | required |
| Audio input | required | required |
| App Sandbox | optional (off for direct) | required |
| Paste automation | enabled | disabled in v1 MAS |
| Sparkle updater | enabled | disabled |
| App Store updates | disabled | enabled |

MAS paste policy decision (resolved):

- MAS v1 uses **clipboard-only output**. No synthetic paste-at-cursor.

## 8.3 Entitlement keys (explicit)

MAS build (`.entitlements`):

- `com.apple.security.app-sandbox = true`
- `com.apple.security.network.client = true`
- `com.apple.security.device.audio-input = true`

Direct build (`.entitlements`):

- App Sandbox disabled
- `com.apple.security.network.client = true`
- `com.apple.security.device.audio-input = true`

Hotkey/paste permission bindings:

- Carbon hotkeys: no additional entitlement key required.
- CGEventTap compatibility mode (direct-only optional): requires user-granted Input Monitoring.
- Paste-at-cursor (direct-only): requires user-granted Accessibility.

## 8.4 Packaging pipeline

Direct pipeline:

1. Build archive.
2. Developer ID sign.
3. Notarize with `notarytool`.
4. Staple ticket.
5. Publish Sparkle appcast.

MAS pipeline:

1. Build with MAS profile.
2. App Store Connect upload.
3. TestFlight beta.
4. staged release.

## 9. Storage Architecture (Locked) and Migration

## 9.1 Final storage design

Base path:

- `~/Library/Application Support/Quedo/`

Structure:

- `db/history.sqlite`
- `media/<session_id>/recording.flac` or `recording.wav` (legacy import preserved)
- `media/<session_id>/transcription.txt` (optional, for exported compatibility)
- `logs/app.log`, `logs/debug.log`
- `exports/diagnostics-<timestamp>.zip`

SQLite tables:

- `schema_migrations`
- `sessions`
- `session_media`
- `session_transcripts`
- `session_events`
- `settings_snapshots`

`sessions` minimum columns:

- `session_id` (UUID)
- `created_at`
- `duration_ms`
- `provider_primary`
- `provider_used`
- `language`
- `output_mode`
- `status`
- `legacy_source_path` (nullable)
- `legacy_audio_format` (`flac|wav|null`)

## 9.2 Exact Python history migration plan

Source path detection:

- default: `~/.local/share/quedo/history/`
- if XDG override env vars exist, resolve equivalent path.

Per entry migration algorithm:

1. Enumerate `YYYY-MM-DD/HHMMSS/` directories.
2. Prefer `recording.flac`; fallback to `recording.wav`.
3. Read `transcription.txt` if present.
4. Create `session_id` deterministically from legacy date+time path hash (idempotent import).
5. Copy audio into new media folder preserving original extension.
6. Insert metadata row into SQLite including `legacy_source_path`.
7. Insert transcript row (if present).
8. Emit `migration_event` row with result.

Rules:

- never mutate legacy files.
- migration is idempotent and resumable.
- if both `.flac` and `.wav` exist, `.flac` marked primary, `.wav` attached as secondary artifact.

## 9.3 History behavior parity commitments

- Replay uses available primary media; if missing flac, wav fallback is automatic.
- Re-transcribe from history works with both flac and wav legacy imports.
- CLI and UI use same history service API.

## 10. Recovery Policy With Hard Numbers

All retries are bounded and instrumented.

## 10.1 Audio capture recovery

- Stream open attempts: **2 total** (initial + one retry) per recording start.
- Retry delay: **300 ms**.
- User feedback on retry: "Audio backend glitch detected. Retrying...".
- On second failure: transition to `degraded(noInputDevice|internalError)`.

Watchdogs:

- Arming watchdog: if `arming` > **1.5 s**, restart audio engine once.
- Recording callback watchdog: if no frame callback for **750 ms**, abort session and transition to retry state.
- Stream stop watchdog: stop must complete within **2.0 s**; otherwise force teardown and continue cleanup.

Cooldowns:

- Mic permission warning cooldown: **300 s**.
- Accessibility warning cooldown: **300 s**.
- Repeated identical degraded notifications cooldown: **120 s**.

## 10.2 Provider recovery

Primary provider policy:

- timeout per request: **12 s** (short clips)
- one transient retry after **1.0 s** backoff
- if still failing with retry-eligible error -> fallback provider

Fallback provider policy:

- one attempt only
- timeout: **12 s**
- if fallback fails -> `retryAvailable`

Provider thrash controls:

- once fallback succeeds, keep fallback as active provider for **30 s cooldown window**.
- re-probe primary health every **60 s** in background.

## 10.3 Hotkey recovery

- Failed registration auto-retry: 1 retry after **300 ms**.
- If still failing: degraded hotkey state and forced remap prompt.
- Re-registration attempt on wake: max **2 attempts** (0.3 s, 1.0 s).

## 10.4 Global recovery budget

- Max automatic recoveries per subsystem: **5 per rolling 10 min**.
- If exceeded: subsystem locks to degraded until user action or 10-minute window reset.

## 11. SLOs Bound to Telemetry and Alerts

## 11.1 Metric definitions (exact)

Metrics emitted locally every session and optionally aggregated (opt-in):

- `hotkey_trigger_total`
- `session_start_total`
- `session_start_failed_total{reason}`
- `session_latency_hotkey_to_first_frame_ms`
- `session_latency_stop_to_final_transcript_ms`
- `provider_primary_attempt_total`
- `provider_primary_success_total`
- `provider_fallback_attempt_total`
- `provider_fallback_success_total`
- `output_success_total{target}`
- `output_failure_total{target,reason}`
- `recovery_attempt_total{subsystem,reason}`
- `recovery_success_total{subsystem,reason}`
- `degraded_enter_total{reason}`
- `crash_unexpected_total`

## 11.2 SLO formulas and targets

1. Hotkey activation success rate:

- formula: `session_start_total / hotkey_trigger_total`
- target: >= **99.9%** per rolling 24h on canary cohort

2. Capture start latency:

- formula: p95 of `session_latency_hotkey_to_first_frame_ms`
- target: <= **150 ms**

3. Stop-to-final transcript latency (<=20s clips):

- formula: p95 of `session_latency_stop_to_final_transcript_ms`
- target: <= **3000 ms**

4. Recovery effectiveness:

- formula: `recovery_success_total / recovery_attempt_total`
- target: >= **95%** for `audio` and `hotkey` subsystems

5. Unexpected termination rate:

- formula: `crash_unexpected_total / active_hours`
- target: <= **0.02 crashes/hour** in soak cohort

## 11.3 Alert thresholds

Alert triggers (internal QA and canary monitoring):

- hotkey success < 99.5% for 3 consecutive 1h windows
- p95 start latency > 200 ms for 3 consecutive 1h windows
- degraded entries > 2.0% of sessions per day
- fallback engaged > 10% of sessions per day (indicates provider instability)
- recovery success < 90% for any subsystem over 24h

## 11.4 Release gates (go/no-go)

Release is blocked if any condition fails:

- all SLO targets unmet in canary 7-day run
- any alert threshold triggered on 2 consecutive days
- soak tests in Section 12 fail pass criteria
- migration failure rate > 0.1%
- parity appendix items marked `Dropped` without approved rationale

## 11.5 Telemetry collection mechanics

Local collection:

- Metrics are emitted as structured events into SQLite table `session_events` with monotonic `event_seq`.
- Latency metrics are computed at session close and written to `session_metrics` rows.
- Counters are rolled up every **60 seconds** into `metrics_rollup_1m`.

Optional remote upload (opt-in only):

- Upload cadence: every **15 minutes** or at app shutdown.
- Payload: aggregated counters and percentiles only (no raw transcript/audio).
- Retry policy: 3 attempts (1s, 5s, 30s), then drop and log.

Alert evaluation:

- During internal dogfood/canary, alert checks run hourly from aggregated telemetry.
- Local developer mode also runs threshold checks and surfaces warnings in Diagnostics.

## 12. Expanded Reliability Test Plan With Pass/Fail Criteria

## 12.1 Long-run soak

Spec:

- 24-hour continuous run on 10 Apple Silicon machines.
- Record/transcribe cycle every 2 minutes with mixed durations (5s/15s/30s).

Pass:

- no deadlocks
- unexpected terminations <= 1 per machine/24h
- hotkey success >= 99.9%

## 12.2 Sleep/wake cycling

Spec:

- 200 automated sleep/wake cycles per machine.
- After each wake, run 3 record/transcribe cycles.

Pass:

- first post-wake capture succeeds >= 99%
- zero manual restart requirement

## 12.3 Hotkey contention under load

Spec:

- CPU load at 80% simulated.
- generate rapid non-target keyboard traffic while triggering hotkey every 10s.

Pass:

- hotkey false negatives <= 0.1%
- zero duplicate session starts

## 12.4 Device hot-swap (unplug/replug)

Spec:

- USB mic unplug/replug 100 cycles.
- Bluetooth headset connect/disconnect 100 cycles.

Pass:

- app recovers route within 3s in >= 99% cycles
- degraded state shown with remediation for remaining cycles

## 12.5 Provider outage simulation

Spec:

- primary provider forced timeout/5xx for 500 consecutive attempts.
- fallback provider available for first 400, then unavailable for 100.

Pass:

- fallback engages automatically in >= 99% eligible attempts
- retryAvailable state shown for dual-failure attempts
- no app hangs; session cleanup always completes

## 12.6 Migration reliability

Spec:

- import 10k legacy entries mixed flac/wav/transcription presence.
- run migration interrupt/resume 20 times.

Pass:

- idempotent final row counts
- no data corruption
- replay/transcribe available for >= 99.99% imported entries

## 12.7 CI enforcement

- each test class has machine-readable report JSON.
- release pipeline parses report and fails build on threshold breach.

## 13. Companion CLI Scope (Parity-Critical)

v1 ships a thin CLI that calls shared app services.

Commands:

- `status`
- `doctor`
- `logs [--stderr|--debug]`
- `transcribe <file> [--language]`
- `history list`
- `history play <n>`
- `history transcribe <n> [--language]`
- `config show`

Intentionally dropped for v1:

- `config edit` interactive shell editor flow (replaced by GUI Settings).

Rationale:

- avoids dual config-authoring surface; CLI remains operational parity tool.

## 14. Linux Shared Contract (Versioned JSON Schema)

## 14.1 Contract artifact

Canonical schema files live in:

- `contracts/schema/session-event-v1.json`
- `contracts/schema/settings-v1.json`
- `contracts/schema/provider-request-v1.json`
- `contracts/schema/provider-response-v1.json`

All payloads include:

- `schema_version` (semantic version string)
- `schema_name`
- `event_id` or `record_id`
- `timestamp_utc`

## 14.2 Compatibility policy

- Minor version (`x.y+1`) may add optional fields only.
- Required field additions require major version bump.
- Field removals/renames require major version bump.
- Consumers must ignore unknown fields.
- Producers must include all required fields for declared version.

Event ordering guarantee:

- each event stream includes monotonic `sequence_number` per session.
- consumers treat out-of-order events as protocol errors and log diagnostics.

## 14.3 Conformance test ownership

- Contract ownership: macOS lead + Linux lead (shared CODEOWNERS on `contracts/schema/`).
- Mac team owns canonical fixture generation.
- Linux team owns secondary fixture replay runner.
- CI requires both runners to pass for any schema change.

Conformance tests:

- schema validation (strict)
- backward-compat read tests (N-1 minor)
- event ordering tests
- golden transcript/output routing tests

## 15. Python -> Swift Strict Parity Appendix (`src/`)

Legend:

- `Kept`: behavior preserved in v1.
- `Changed`: preserved with intentional architectural difference.
- `Dropped`: intentionally removed in v1.

## 15.1 `src/quedo/env.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| `VALID_WHISPER_MODELS` allowlist | Provider/model allowlists in `SettingsService` per adapter | Changed | Expanded to provider-scoped model lists |
| `ConfigError` + `ConfigErrors` multi-error aggregation | `SettingsValidationErrorSet` aggregates all invalid fields before apply | Kept | Same UX principle: show all errors at once |
| strict hotkey parsing (`modifier+key`, known modifier set, unknown key error) | `HotkeyParser` strict grammar and key enum validation | Kept | Reject unknown tokens with field-level errors |
| `TRANSCRIPTION_OUTPUT=none` semantics | Output mode enum includes `none` and disables all output targets | Kept | Explicitly tested in unit + onboarding |
| `TRANSCRIPTION_LANGUAGE=auto` -> `None` | Language setting includes `auto` sentinel | Kept | Serialized as null in runtime settings |
| `GROQ_TIMEOUT` positive int validation | Provider timeout validation with min/max bounds | Kept | min 1s, max 120s |
| vocabulary comma parsing | `vocabularyHints[]` normalized and trimmed | Kept | Empty tokens removed |

## 15.2 `src/quedo/paths.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| XDG config/data/state dirs | macOS Library paths + migration from XDG legacy | Changed | Native macOS pathing for app distribution |
| create config with defaults and `0600` | Defaults in `UserDefaults`; secrets in Keychain; file permissions for logs/media locked to user | Changed | More secure split for secret vs non-secret |
| history/log/pid path helpers | `HistoryService` and `DiagnosticsService` path providers | Changed | No pid file model in single-process app |

## 15.3 `src/quedo/permissions.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| Accessibility check | `PermissionService.checkAccessibility()` | Kept | direct build only for paste mode |
| Input Monitoring preflight check | `PermissionService.checkInputMonitoring()` | Changed | only used if CGEventTap compatibility mode enabled |
| mic probe quick check | onboarding mic loopback + runtime probe endpoint | Kept | stricter pass/fail + remediation loop |
| `check_all()` aggregation | `PermissionSnapshot` struct with all flags | Kept | used by doctor UI/CLI |
| open settings by pane | `openSystemSettings(pane)` with deep links | Kept | same user affordance |

## 15.4 `src/quedo/log_config.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| console + debug/info rotating logs | `OSLog` + rolling log mirror files | Changed | preserves tail/export workflows |
| suppress noisy HTTP info logs | provider logger category filters | Kept | noise suppression retained |

## 15.5 `src/quedo/packages/audio_recorder/main.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| recorder lock to prevent concurrent session start | single active session guard in `AppController` | Kept | hard invariant |
| prewarm audio backend | `AudioService.prepareEngine()` at boot and wake | Kept | AVAudioEngine warm-up |
| stream open retry with escalating resets | **one retry with user feedback before failure** | Changed | exact parity for user-visible retry; AVFoundation-specific reinit replaces PortAudio reset internals |
| fallback sample rates | hardware-native format negotiation | Changed | no manual fixed fallback table in AVFoundation path |
| failure classification (`stream_open_error`, etc.) | typed error codes (`capture_open_failed`, `empty_audio`, `cancelled`) | Kept | mapped in diagnostics |
| stream close timeout guard | stop watchdog 2.0s force teardown | Kept | concrete timeout retained |
| device diagnostics on failures | route/device snapshot event logging | Kept | includes selected input and available devices |

## 15.6 `src/quedo/packages/keyboard_listener/main.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| register/unregister hotkeys | `HotkeyService.setBindings()` | Kept | dynamic updates supported |
| callback isolation with error logging | hotkey callback dispatch via actor + guarded error handling | Kept | no callback crash propagation |
| start/stop listener lifecycle | `HotkeyService.activate()/deactivate()` | Kept | tied to app phase |

## 15.7 `src/quedo/packages/notifications/main.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| info/error notifications | UserNotifications-based alerts | Kept | copy standardized by UI state matrix |
| sound playback cues | configurable start/process/success/error sounds | Kept | defaults map to current cues |

## 15.8 `src/quedo/packages/transcriber/main.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| transcribe in-memory arrays | `TranscriptionService.transcribe(sessionAudio)` | Kept | primary runtime path |
| file transcription path | CLI `transcribe <file>` + service file input path | Kept | same capability |
| video/unsupported audio conversion via ffmpeg | converter utility for file transcription mode | Kept | not used for live mic path |
| long audio chunking thresholds | provider policy chunking config | Kept | configurable, defaults aligned |
| rolling context prompt between chunks | chunk prompt carry-forward | Kept | same algorithmic intent |
| hallucination pattern cleanup | deterministic cleanup rules | Kept | v1 includes pattern list |
| request-too-large handling | auto chunk shrink + actionable error | Kept | same user guidance intent |
| **chunk boundary diagnostic context logs** | boundary context logs persisted in diagnostics | Kept | explicitly preserved per review finding |

## 15.9 `src/quedo/main.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| toggle hotkey start/stop | default toggle mode | Kept | default behavior unchanged |
| cancel recording hotkey | cancel action in recording/processing phases | Kept | state-aware cancellation |
| retry last transcription hotkey | retry action from `retryAvailable` | Kept | preserved |
| recording lock and worker thread orchestration | single lifecycle actor + async tasks | Changed | concurrency model modernized |
| silent recording warning with cooldown | same warning with 300s cooldown | Kept | same timing |
| accessibility warning with cooldown | same warning with 300s cooldown | Kept | same timing |
| copy to clipboard output | `OutputService` clipboard target | Kept | mandatory fallback path |
| paste at cursor via cmd+v simulation | direct build only paste target | Changed | MAS profile disables |
| history save async | `HistoryService.saveSession()` async non-blocking | Kept | no user-blocking persistence |
| permission checks on startup for hotkeys/paste | boot checks + degraded state mapping | Kept | more explicit state transition |

## 15.10 `src/quedo/cli.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| `--version` with upgrade hint | CLI `--version` with channel-specific upgrade text | Kept | direct/MAS text differs |
| interactive `init` wizard | GUI onboarding + CLI `doctor/setup-status` | Changed | GUI-first reliability path |
| `start/stop/restart/status` daemon commands | `status` retained; start/stop/restart map to app launch/control commands | Changed | single-process app, no pid daemon model |
| startup crash detection via stderr log | launch health checks + diagnostics incident markers | Changed | equivalent user-visible behavior |
| `doctor --fix` | `doctor` CLI and Diagnostics UI with open-settings actions | Kept | parity preserved |
| `logs --stderr` | `logs` with `--debug`/`--error` channels | Changed | log channel names modernized |
| `transcribe <file>` | same command via shared transcription service | Kept | includes language option |
| `history list` | same command | Kept | SQLite-backed output |
| `history transcribe N` | same command with N=latest index semantics | Kept | semantics preserved |
| `history play N` | same command; flac/wav fallback replay | Kept | explicit fallback retained |
| `config show` | same command (prints non-secret settings) | Kept | secrets masked |
| `config edit` interactive editor | removed (GUI settings is source of truth) | Dropped | intentional simplification |

## 15.11 `src/quedo/__init__.py` and package `__init__.py`

| Python Behavior | Swift v2 Mapping | Status | Notes |
|---|---|---|---|
| exposed package version | app/CLI semantic version constant | Kept | shared by app and CLI |
| package export convenience wrappers | module namespace in Swift packages | Changed | language-level module model differs |

## 16. Implementation Order (No Open Decisions Remaining)

1. Scaffold single-process app shell + lifecycle state model.
2. Build onboarding with reliability gates (Section 7).
3. Implement hotkey service (Carbon only) and deterministic UI matrix.
4. Implement audio capture path with bounded retry/watchdogs.
5. Implement provider path (Groq primary + OpenAI fallback).
6. Implement output service (clipboard + direct-build paste).
7. Implement SQLite/media history and Python migration importer.
8. Implement diagnostics + doctor UI/CLI.
9. Complete reliability suite and enforce release gates.
10. Ship direct v1; begin MAS profile adaptation.

No architectural TBD items remain.

## 17. Reviewer Finding Closure Matrix

| Reviewer Finding | Resolution in v2 |
|---|---|
| Resolve open decisions (Section 23) | Section 2 freezes distribution, hotkeys, offline provider, hardware baseline, default interaction mode, post-processing scope, MAS paste policy |
| Build-profile hotkey matrix | Sections 2.2 and 8.2 define backend per profile, fallback rules, and permission implications |
| Strict Python parity mapping | Section 15 maps `src/` behavior-by-behavior with `Kept/Changed/Dropped` status |
| Config validation parity gaps | Section 15.1 explicitly preserves multi-error aggregation, strict hotkey parsing, `TRANSCRIPTION_OUTPUT=none` |
| Stream-open retry parity gap | Sections 10.1 and 15.5 specify one retry with user feedback before hard fail |
| Chunk boundary diagnostics under-specified | Section 15.8 explicitly keeps boundary context logs in diagnostics |
| Storage ambiguity | Section 2.7 and Section 9 lock SQLite + media and define exact migration |
| Overlapping state machines over-engineered | Section 5 replaces with single primary lifecycle model and derived sub-state |
| SLOs not measurable/operationalized | Section 11 defines exact metrics, formulas, thresholds, release gates, and collection mechanics |
| UI transitions not deterministic | Section 6 provides exact icon/copy/sound/actions per state |
| First-run reliability checks missing | Section 7 defines hotkey test, mic loopback, provider test, with remediation loops |
| Recovery numbers missing | Section 10 defines retries, watchdogs, cooldowns, backoff ceilings, and recovery budgets |
| Reliability test plan insufficient | Section 12 adds soak/sleep/hotkey/device/outage/migration tests with hard pass/fail criteria |
| Linux contract insufficient | Section 14 defines versioned JSON schemas, compatibility rules, ordering guarantees, and ownership |
