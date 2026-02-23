# Architecture Review

## 1. Executive Summary
The architecture is directionally strong but not implementation-ready. It solves many Python pain points in principle, yet it currently leaves critical product and platform decisions unresolved, under-specifies several parity behaviors from the existing codebase, and introduces avoidable complexity in state management.

The biggest risk is not “bad ideas”; it is **decision debt**: unresolved App Store hotkey behavior, offline provider policy, and hardware baseline choices can stall implementation or force expensive rewrites mid-build. If you start coding now, you will likely ship a partial rewrite that regresses reliability or UX.

## 2. Completeness Gaps
- Current Python config validation depth is not fully represented. The existing app performs strict hotkey parsing, aggregates multiple config errors before startup, and supports explicit `TRANSCRIPTION_OUTPUT=none` semantics (`src/whisper_assistant/env.py`). The architecture’s parity list is too high-level and risks dropping these guardrails.
- Python has a concrete stream recovery path: retry once on `stream_open_error` with user feedback before hard failure (`src/whisper_assistant/main.py`). The architecture describes recovery conceptually but does not clearly preserve this behavior.
- Migration coverage for history/media is incomplete. Current history workflows rely on `recording.flac` with `.wav` fallback and CLI playback/transcribe flows (`src/whisper_assistant/main.py`, `src/whisper_assistant/cli.py`). The migration section should explicitly preserve both file variants and behaviors.
- Chunking diagnostics are under-specified. The current transcriber logs contextual windows around chunk boundaries to debug cut-word issues (`src/whisper_assistant/packages/transcriber/main.py`). The architecture mentions chunking, but not this diagnostic capability.
- `rewrite-analysis.md` and `architecture.md` are not fully aligned on storage trajectory. One still leaves room for folder semantics while the architecture commits to SQLite metadata + media files. This must be normalized before implementation.
- Linux companion contract detail is insufficient for execution. “Shared contracts” are referenced, but concrete schema versioning, compatibility policy, and conformance test ownership are not fully pinned.

## 3. Feasibility Concerns
- Global hotkey backend strategy is risky. The architecture leans on Carbon `RegisterEventHotKey` while also targeting MAS-safe behavior. Without explicit build-profile/backend rules and entitlement constraints, this is a likely App Store or runtime failure point.
- Key open decisions are parked in Section 23 and directly affect implementation shape: offline provider choice, MAS paste-automation behavior, Intel support, hold-to-talk default, and post-processing scope. These are not “later” decisions; they are architecture decisions.
- Reliability SLOs are defined (hotkey success and latency), but observability is not tied to enforceable error budgets and alert thresholds. You cannot prove reliability gains without measurable pass/fail criteria.
- Streaming/partial transcription + fallback behavior is not tightly mapped to UI state transitions. Implementation teams will infer different behavior and produce inconsistent UX.

## 4. UX Improvements
- Add a true first-run reliability path, not just permission prompts: hotkey test, microphone loopback check, provider connectivity check, and explicit remediation steps before enabling “ready” state.
- Define deterministic UI state mapping for: `recording`, `processing`, `streaming partial`, `provider fallback in progress`, `degraded mode`, and `retry available`.
- Specify user-visible recovery microcopy for top failures (permission denied, no input device, provider timeout, fallback provider engaged, paste blocked by app/sandbox policy).
- Preserve power-user parity from Python tooling in product form: diagnostics visibility, quick retry, history replay/transcribe, and actionable doctor-style checks.
- Clarify menu bar interaction under failure: whether users can force-stop recording, re-run checks, or switch provider without opening a deep settings flow.

## 5. Reliability Gaps
- Failure matrix needs explicit coverage for runtime device churn (mic unplug/replug), stream-open transient failures with bounded retry policy, sleep/wake race windows, and provider failover loops.
- Self-recovery is described, but watchdog triggers, cooldown windows, and backoff ceilings need hard numbers to avoid recovery thrash.
- Reliability instrumentation is not coupled to rollout gates. Add go/no-go thresholds for soak tests and pre-release confidence criteria.
- Contract tests for Linux/macOS parity are not specified deeply enough (schema compatibility, event ordering guarantees, and backward compatibility behavior across versions).

## 6. Over-engineering Flags
- Four overlapping state machines (global/recording/permission/provider) plus actor/event bus layering is high-complexity for v1. This increases synchronization risk and debugging cost.
- Provider abstraction appears to optimize for long-term optionality before proving core reliability. v1 should prioritize a minimal path that is robust under one primary provider + one fallback.
- Running menu bar UX, command palette, extensive fallback orchestration, and broad diagnostics all at once increases integration surface unnecessarily.

## 7. Recommended Changes
1. Resolve all Section 23 open decisions immediately and freeze them as architecture inputs.
2. Define a build-profile matrix for hotkey backends (MAS vs direct), with entitlement requirements and runtime fallback rules.
3. Add a strict parity appendix mapping each existing Python behavior in `src/` to its Swift equivalent (or intentional removal).
4. Lock storage architecture decision (SQLite + media files vs folder semantics) and add an explicit migration plan for existing `.flac/.wav` history artifacts.
5. Bind SLOs to concrete telemetry definitions, alert thresholds, and release gates.
6. Simplify state management for v1: reduce to one primary lifecycle state model with derived sub-states where possible.
7. Specify UI state transitions and user-facing copy for streaming/fallback/recovery paths so implementation is deterministic.
8. Expand reliability test plan with long-run soak, sleep/wake cycling, hotkey contention, device churn, provider outage drills, and automated pass/fail criteria.
9. Define the Linux shared contract as a versioned schema with compatibility rules and cross-platform conformance tests before companion implementation starts.
10. Treat first-run experience as a reliability feature: add setup validation checks and remediation loops before declaring the app ready.
