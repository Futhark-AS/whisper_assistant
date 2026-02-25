# Review Summary — Quedo macOS

Synthesized from 5 parallel review agents (2026-02-25).

## 🔴 Critical / Must-Fix

### 1. State machine blocks retry transition
- **Source:** bug-hunter
- **Files:** `StateMachine.swift:320-365`, `AppController.swift:322-335`
- `.processing → .retryAvailable` is disallowed in `isAllowedTransition`. The `try?` silently swallows the failure, leaving UI stuck in processing state forever on transcription errors.
- **Fix:** Add `.processing → .retryAvailable` to allowed transitions.

### 2. Temp files never cleaned up
- **Source:** bug-hunter + cleanup
- **Files:** `TranscriptionPipeline.swift:200-260`, `AudioCaptureEngine.swift:172-174`
- Chunk WAV files in `~/tmp/QuedoChunks` and capture files in `QuedoCapture` are created but never deleted. Disk grows unbounded with usage.
- **Fix:** `defer` cleanup in `runChunks`, delete source temp files after `persistAudioArtifact` copy.

### 3. Corrupt SQLite DB = fatal crash
- **Source:** robustness
- **Files:** `HistoryStore.swift:73-85`, `AppDelegate.swift:47-59`
- DB open/schema failure crashes the app. No recovery.
- **Fix:** Detect corruption, quarantine bad DB (rename with timestamp), recreate fresh, continue in degraded mode.

## 🟠 High Priority

### 4. Onboarding gates are superficial
- **Source:** bug-hunter + robustness
- **Files:** `OnboardingCoordinator.swift:41-90`, `TranscriptionPipeline.swift:127-135`
- Architecture Section 7 requires: 20s hotkey verification (two consecutive triggers within 200ms), loopback playback confirmation, synthetic 2s provider test with 6s timeout. Currently just checks if a binding exists.
- **Fix:** Implement real verification loops per spec.

### 5. Mid-session disruptions unhandled
- **Source:** robustness
- **Files:** `AudioCaptureEngine.swift:206-244`, `PermissionCoordinator.swift:43-80`
- Mic disconnect, route change, sleep/wake, permission revocation during recording: errors logged but no deterministic state transition or user recovery flow.
- **Fix:** Route events into AppController state transitions, cancel active recording, transition to explicit degraded/retry states with reason codes.

### 6. Recovery policies not wired
- **Source:** robustness
- **Files:** `AudioCaptureEngine.swift:116-205`, `DiagnosticsCenter.swift:93-145`
- Section 10 watchdog failures, cooldown policies (300s mic warning, 120s degraded warning), and recovery budget (5 per rolling 10min) exist as concepts but aren't connected to runtime flows.
- **Fix:** Wire watchdog failures to lifecycle transitions, persist cooldown timestamps, invoke budget checks in every auto-recovery path.

### 7. SLO telemetry mostly missing
- **Source:** robustness
- **Files:** `AppController.swift:183-335`, `DiagnosticsCenter.swift:93-162`
- Most Section 11 metrics absent: provider attempts/success, fallback usage, output success/failure, recovery effectiveness, first-frame latency. Upload loop defined but unused.
- **Fix:** Instrument pipeline/provider/fallback/output/recovery boundaries. Start upload loop on opt-in.

### 8. AppController is a god object
- **Source:** SOLID
- **Files:** `AppController.swift` (~450 lines)
- Mixes lifecycle, settings bootstrap, permissions, hotkey setup, recording/transcription, diagnostics, onboarding, and UI/menu updates.
- **Fix:** Extract `RecordingFlowController`, `MenuActionHandler`, `PermissionRecovery` protocols and implementations. AppController becomes thin coordinator.

### 9. Providers are copy-pasted
- **Source:** DRY
- **Files:** `OpenAIProvider.swift:30-137`, `GroqProvider.swift:30-137`
- ~90% identical: multipart construction, URLRequest setup, status code handling, MIME mapping, response DTO.
- **Fix:** Extract `MultipartTranscriptionClient` base. Providers become thin config wrappers (endpoint URL, provider kind).

## 🟡 Medium

### 10. No offline/misconfigured startup handling
- **Source:** robustness
- Can enter recording flow without preflight validation. Missing key or no network only fails later as generic retry.
- **Fix:** Add readiness preflight before entering `ready` state.

### 11. PreferencesViewModel does too many things
- **Source:** SOLID
- **File:** `PreferencesView.swift:600-1070`
- Mixes UI state, settings persistence, API key CRUD, clipboard, hotkey validation.
- **Fix:** Extract `PreferencesSettingsStore`, `APIKeyStore`, `HotkeyValidator`.

### 12. Hardcoded hotkey presets violate open/closed
- **Source:** SOLID
- **File:** `PreferencesView.swift:820-952`
- Adding a preset requires modifying existing methods.
- **Fix:** Data-driven preset registry.

### 13. No dependency injection
- **Source:** SOLID
- High-level modules directly instantiate low-level deps. Testing requires full app context.
- **Fix:** Inject protocols at init boundaries.

### 14. Critical runtime paths have zero tests
- **Source:** cleanup
- No tests for: `AudioCaptureEngine`, `HotkeyManager`, `OutputRouter`, `DiagnosticsCenter`, `PermissionCoordinator`.
- **Fix:** Add focused unit tests with protocol mocks.

### 15. Dead code: describeHotkey in PreferencesView
- **Source:** cleanup
- **File:** `PreferencesView.swift:962-971`
- No call sites. Remove or wire in.
