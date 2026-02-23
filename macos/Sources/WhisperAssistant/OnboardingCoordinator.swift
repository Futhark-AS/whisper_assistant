import Foundation
import WhisperAssistantCore

/// Result of onboarding reliability gate execution.
struct OnboardingGateResult {
    /// Indicates all required gates passed.
    let passed: Bool
    /// Degraded reason when gate fails.
    let degradedReason: DegradedReason
}

/// Handles first-run onboarding reliability checks.
final class OnboardingCoordinator {
    private enum Constants {
        static let completedKey = "whisper.assistant.onboarding.completed"
    }

    private let userDefaults: UserDefaults

    init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    func requiresOnboarding() async -> Bool {
        !userDefaults.bool(forKey: Constants.completedKey)
    }

    func markCompleted() {
        userDefaults.set(true, forKey: Constants.completedKey)
    }

    func runReliabilityGates(
        settings: AppSettings,
        hotkeyManager _: HotkeyManager,
        audioEngine: AudioCaptureEngine,
        pipeline: TranscriptionPipeline
    ) async -> OnboardingGateResult {
        let hotkeyPassed = runHotkeyVerification(settings: settings)
        if !hotkeyPassed {
            return OnboardingGateResult(passed: false, degradedReason: .hotkeyFailure)
        }

        let micPassed = await runMicrophoneLoopback(audioEngine: audioEngine)
        if !micPassed {
            return OnboardingGateResult(passed: false, degradedReason: .noInputDevice)
        }

        let connectivity = await pipeline.connectivityCheck(primary: settings.provider.primary, fallback: settings.provider.fallback)
        if !(connectivity.primaryOK || connectivity.fallbackOK) {
            return OnboardingGateResult(passed: false, degradedReason: .providerUnavailable)
        }

        if settings.outputMode == .none {
            return OnboardingGateResult(passed: false, degradedReason: .internalError)
        }

        markCompleted()
        return OnboardingGateResult(passed: true, degradedReason: .internalError)
    }

    private func runHotkeyVerification(settings: AppSettings) -> Bool {
        let requiredActions: Set<String> = ["toggle", "retry", "cancel"]
        let configured = Set(settings.hotkeys.map(\.actionID))
        return requiredActions.isSubset(of: configured)
    }

    private func runMicrophoneLoopback(audioEngine: AudioCaptureEngine) async -> Bool {
        let sessionID = UUID()
        do {
            try await audioEngine.startRecording(sessionID: sessionID)
            try? await Task.sleep(for: .seconds(2))
            let result = try await audioEngine.stopRecording()
            return result.durationMS >= 1800
        } catch {
            await audioEngine.cancelRecording()
            return false
        }
    }
}
