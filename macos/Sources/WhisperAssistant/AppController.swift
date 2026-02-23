import Foundation
import WhisperAssistantCore

/// Application coordinator actor and single lifecycle owner.
actor AppControllerActor {
    private let configurationManager: ConfigurationManager
    private let lifecycle: LifecycleStateMachine
    private let permissionCoordinator: PermissionCoordinator
    private let audioEngine: AudioCaptureEngine
    private let transcriptionPipeline: TranscriptionPipeline
    private let outputRouter: OutputRouter
    private let historyStore: HistoryStore
    private let diagnostics: DiagnosticsCenter
    private let hotkeyManager: HotkeyManager
    private let onboardingCoordinator: OnboardingCoordinator

    private let uiUpdate: @MainActor @Sendable (AppLifecycleSnapshot, UIStateContract) -> Void

    private var settings: AppSettings = .default
    private var isRecording = false
    private var latestAudio: AudioCaptureResult?

    init(
        configurationManager: ConfigurationManager,
        lifecycle: LifecycleStateMachine,
        permissionCoordinator: PermissionCoordinator,
        audioEngine: AudioCaptureEngine,
        transcriptionPipeline: TranscriptionPipeline,
        outputRouter: OutputRouter,
        historyStore: HistoryStore,
        diagnostics: DiagnosticsCenter,
        hotkeyManager: HotkeyManager,
        onboardingCoordinator: OnboardingCoordinator,
        uiUpdate: @escaping @MainActor @Sendable (AppLifecycleSnapshot, UIStateContract) -> Void
    ) {
        self.configurationManager = configurationManager
        self.lifecycle = lifecycle
        self.permissionCoordinator = permissionCoordinator
        self.audioEngine = audioEngine
        self.transcriptionPipeline = transcriptionPipeline
        self.outputRouter = outputRouter
        self.historyStore = historyStore
        self.diagnostics = diagnostics
        self.hotkeyManager = hotkeyManager
        self.onboardingCoordinator = onboardingCoordinator
        self.uiUpdate = uiUpdate
    }

    func boot() async {
        do {
            settings = try await configurationManager.loadSettings()
            try await applyHotkeyBindings()

            await audioEngine.prepareEngine()
            let permissionSnapshot = await permissionCoordinator.checkAll()

            if !permissionSnapshot.isFullyReady {
                try await lifecycle.transition(to: .degraded, degradedReason: .permissions)
                await diagnostics.emit(
                    DiagnosticEvent(name: "degraded_enter_total", sessionID: nil, attributes: ["reason": "permissions"])
                )
                await pushUI()
                return
            }

            if await onboardingCoordinator.requiresOnboarding() {
                try await lifecycle.transition(to: .onboarding)
                await pushUI()
                let result = await onboardingCoordinator.runReliabilityGates(
                    settings: settings,
                    hotkeyManager: hotkeyManager,
                    audioEngine: audioEngine,
                    pipeline: transcriptionPipeline
                )
                if result.passed {
                    try await lifecycle.transition(to: .ready)
                } else {
                    try await lifecycle.transition(to: .degraded, degradedReason: result.degradedReason)
                }
                await pushUI()
                return
            }

            try await lifecycle.transition(to: .ready)
            await pushUI()
        } catch {
            await diagnostics.emit(
                DiagnosticEvent(
                    name: "degraded_enter_total",
                    sessionID: nil,
                    attributes: ["reason": "internalError", "error": String(describing: error)]
                )
            )
            do {
                try await lifecycle.transition(to: .degraded, degradedReason: .internalError)
            } catch {
                // Keep current phase if transition fails.
            }
            await pushUI()
        }
    }

    func reloadSettingsFromDisk() async {
        do {
            settings = try await configurationManager.loadSettings()
            try await applyHotkeyBindings()
            await diagnostics.emit(
                DiagnosticEvent(name: "settings_reloaded", sessionID: nil, attributes: ["source": "preferences"])
            )
            await pushUI()
        } catch {
            await diagnostics.emit(
                DiagnosticEvent(
                    name: "settings_reload_failed",
                    sessionID: nil,
                    attributes: ["error": String(describing: error)]
                )
            )
        }
    }

    func handleMenuAction(_ action: AppAction) async {
        switch action {
        case .startRecording:
            await startRecordingFlow()
        case .stop:
            await stopRecordingFlow()
        case .cancel:
            await cancelFlow()
        case .retry:
            await retryFlow()
        case .useClipboardOnly:
            settings.outputMode = .clipboard
            do {
                try await configurationManager.saveSettings(settings)
                try await lifecycle.transition(to: .ready)
            } catch {
                await diagnostics.emit(
                    DiagnosticEvent(name: "settings_save_error", sessionID: nil, attributes: ["error": String(describing: error)])
                )
            }
            await pushUI()
        case .refreshDevices:
            await audioEngine.prepareEngine()
            await pushUI()
        case .runChecks:
            await runChecksFlow()
        case .retryRegistration, .rebindHotkey:
            await retryHotkeyRegistrationFlow()
        default:
            await diagnostics.emit(
                DiagnosticEvent(name: "menu_action", sessionID: nil, attributes: ["action": action.rawValue])
            )
        }
    }

    func handleHotkey(actionID: String) async {
        await diagnostics.recordMetric(MetricPoint(name: "hotkey_trigger_total", value: 1, tags: ["action": actionID]))

        switch actionID {
        case "toggle":
            if isRecording {
                await stopRecordingFlow()
            } else {
                await startRecordingFlow()
            }
        case "retry":
            await retryFlow()
        case "cancel":
            await cancelFlow()
        default:
            break
        }
    }

    func shutdown() async {
        await audioEngine.cancelRecording()
        hotkeyManager.deactivate()
        try? await lifecycle.transition(to: .shuttingDown)
        await pushUI()
    }

    private func startRecordingFlow() async {
        if isRecording {
            return
        }

        let sessionID = UUID()
        do {
            try await lifecycle.beginSession(id: sessionID)
            try await lifecycle.transition(to: .arming)
            await pushUI()

            try await audioEngine.startRecording(sessionID: sessionID)
            try await lifecycle.transition(to: .recording)
            isRecording = true
            await diagnostics.recordMetric(MetricPoint(name: "session_start_total", value: 1, tags: [:]))
            await pushUI()
        } catch let error as AudioCaptureError {
            let degraded: DegradedReason = (error == .noInputDevice) ? .noInputDevice : .internalError
            try? await lifecycle.transition(to: .degraded, degradedReason: degraded)
            await lifecycle.setLastErrorCode("capture_open_failed")
            await diagnostics.recordMetric(
                MetricPoint(name: "session_start_failed_total", value: 1, tags: ["reason": "capture_open_failed"])
            )
            await pushUI()
            await lifecycle.endSession()
        } catch {
            try? await lifecycle.transition(to: .degraded, degradedReason: .internalError)
            await pushUI()
            await lifecycle.endSession()
        }
    }

    private func stopRecordingFlow() async {
        guard isRecording else {
            return
        }

        do {
            try await lifecycle.transition(to: .processing)
            await pushUI()

            let capture = try await audioEngine.stopRecording()
            latestAudio = capture
            isRecording = false

            let start = Date()
            let pipelineResult = try await transcriptionPipeline.transcribe(audioFileURL: capture.fileURL, settings: settings)

            if pipelineResult.fallbackUsed {
                try? await lifecycle.transition(to: .providerFallback)
                await pushUI()
                await lifecycle.markFallbackAttempted()
            }

            try await lifecycle.transition(to: .outputting)
            await pushUI()

            _ = try await outputRouter.route(text: pipelineResult.text, mode: settings.outputMode, profile: settings.buildProfile)

            let sessionID = capture.sessionID
            let record = SessionRecord(
                sessionID: sessionID,
                createdAt: Date(),
                durationMS: capture.durationMS,
                providerPrimary: settings.provider.primary,
                providerUsed: pipelineResult.providerUsed,
                language: settings.language,
                outputMode: settings.outputMode,
                status: .success,
                transcript: pipelineResult.text,
                audioPath: capture.fileURL
            )

            try await historyStore.saveSession(record)
            await diagnostics.recordMetric(
                MetricPoint(
                    name: "session_latency_stop_to_final_transcript_ms",
                    value: Date().timeIntervalSince(start) * 1000,
                    tags: [:]
                )
            )

            try await lifecycle.transition(to: .ready)
            await lifecycle.endSession()
            await pushUI()
        } catch {
            isRecording = false
            try? await lifecycle.transition(to: .retryAvailable)
            await lifecycle.setLastErrorCode("pipeline_failed")
            await lifecycle.endSession()
            await pushUI()
        }
    }

    private func retryFlow() async {
        guard let latestAudio else {
            return
        }

        do {
            try await lifecycle.transition(to: .processing)
            await pushUI()

            let result = try await transcriptionPipeline.transcribe(audioFileURL: latestAudio.fileURL, settings: settings)
            try await lifecycle.transition(to: .outputting)
            await pushUI()

            _ = try await outputRouter.route(text: result.text, mode: settings.outputMode, profile: settings.buildProfile)

            try await lifecycle.transition(to: .ready)
            await lifecycle.endSession()
            await pushUI()
        } catch {
            try? await lifecycle.transition(to: .retryAvailable)
            await lifecycle.endSession()
            await pushUI()
        }
    }

    private func cancelFlow() async {
        await audioEngine.cancelRecording()
        isRecording = false
        await lifecycle.endSession()
        try? await lifecycle.transition(to: .ready)
        await pushUI()
    }

    private func retryHotkeyRegistrationFlow() async {
        do {
            try await applyHotkeyBindings()
            await lifecycle.setLastErrorCode(nil)
            try? await lifecycle.transition(to: .ready)
        } catch {
            await lifecycle.setLastErrorCode("hotkey_registration_failed")
            try? await lifecycle.transition(to: .degraded, degradedReason: .hotkeyFailure)
            await diagnostics.emit(
                DiagnosticEvent(
                    name: "hotkey_registration_retry_failed",
                    sessionID: nil,
                    attributes: ["error": String(describing: error)]
                )
            )
        }
        await pushUI()
    }

    private func runChecksFlow() async {
        let permissions = await permissionCoordinator.checkAll()
        let connectivity = await transcriptionPipeline.connectivityCheck(primary: settings.provider.primary, fallback: settings.provider.fallback)

        var hotkeysReady = true
        do {
            try await applyHotkeyBindings()
        } catch {
            hotkeysReady = false
            await diagnostics.emit(
                DiagnosticEvent(
                    name: "run_checks_hotkey_registration_failed",
                    sessionID: nil,
                    attributes: ["error": String(describing: error)]
                )
            )
        }

        await diagnostics.emit(
            DiagnosticEvent(
                name: "run_checks_completed",
                sessionID: nil,
                attributes: [
                    "microphone": permissions.microphone.rawValue,
                    "accessibility": permissions.accessibility.rawValue,
                    "inputMonitoring": permissions.inputMonitoring.rawValue,
                    "providerPrimaryOK": connectivity.primaryOK ? "true" : "false",
                    "providerFallbackOK": connectivity.fallbackOK ? "true" : "false",
                    "hotkeysReady": hotkeysReady ? "true" : "false"
                ]
            )
        )

        if !permissions.isFullyReady {
            await lifecycle.setLastErrorCode("permissions_not_ready")
            try? await lifecycle.transition(to: .degraded, degradedReason: .permissions)
            await pushUI()
            return
        }

        if !hotkeysReady {
            await lifecycle.setLastErrorCode("hotkey_registration_failed")
            try? await lifecycle.transition(to: .degraded, degradedReason: .hotkeyFailure)
            await pushUI()
            return
        }

        if !(connectivity.primaryOK || connectivity.fallbackOK) {
            await lifecycle.setLastErrorCode("provider_connectivity_failed")
            try? await lifecycle.transition(to: .degraded, degradedReason: .providerUnavailable)
            await pushUI()
            return
        }

        await lifecycle.setLastErrorCode(nil)
        try? await lifecycle.transition(to: .ready)
        await pushUI()
    }

    private func pushUI() async {
        let snapshot = await lifecycle.snapshot()
        let contract = await lifecycle.uiContract()
        await uiUpdate(snapshot, contract)
    }

    private func applyHotkeyBindings() async throws {
        try await hotkeyManager.setBindings(settings.hotkeys) { [weak self] action in
            Task {
                await self?.handleHotkey(actionID: action)
            }
        }
    }
}
