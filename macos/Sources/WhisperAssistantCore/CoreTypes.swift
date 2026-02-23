import Foundation

/// Distribution profile that controls feature availability.
public enum BuildProfile: String, Codable, Sendable {
    /// Direct Developer ID distribution.
    case direct
    /// Mac App Store distribution.
    case mas
}

/// Output destination mode for finalized transcripts.
public enum OutputMode: String, Codable, Sendable, CaseIterable {
    /// Do not emit output.
    case none
    /// Copy transcript to clipboard only.
    case clipboard
    /// Paste transcript at the current cursor location.
    case pasteAtCursor
    /// Copy and paste transcript.
    case clipboardAndPaste
}

/// Available provider families for transcription.
public enum ProviderKind: String, Codable, Sendable, CaseIterable {
    /// Groq hosted model endpoints.
    case groq
    /// OpenAI hosted model endpoints.
    case openAI
}

/// Mode for handling recording with hotkeys.
public enum RecordingInteractionMode: String, Codable, Sendable, CaseIterable {
    /// Press once to start and once to stop.
    case toggle
    /// Hold key to record while pressed.
    case hold
}

/// Supported hotkey modifiers.
public struct HotkeyModifiers: OptionSet, Codable, Sendable, Hashable {
    /// Raw bitmask value.
    public let rawValue: UInt32

    /// Creates a modifier bitmask.
    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    /// Command key.
    public static let command = HotkeyModifiers(rawValue: 1 << 0)
    /// Option/alt key.
    public static let option = HotkeyModifiers(rawValue: 1 << 1)
    /// Control key.
    public static let control = HotkeyModifiers(rawValue: 1 << 2)
    /// Shift key.
    public static let shift = HotkeyModifiers(rawValue: 1 << 3)
}

/// A concrete hotkey binding.
public struct HotkeyBinding: Codable, Hashable, Sendable {
    /// Stable action identifier.
    public let actionID: String
    /// macOS virtual key code.
    public let keyCode: UInt32
    /// Required key modifiers.
    public let modifiers: HotkeyModifiers

    /// Creates a new hotkey binding.
    public init(actionID: String, keyCode: UInt32, modifiers: HotkeyModifiers) {
        self.actionID = actionID
        self.keyCode = keyCode
        self.modifiers = modifiers
    }
}

/// Provider credentials and request-time settings.
public struct ProviderConfiguration: Codable, Sendable {
    /// Primary provider to use for normal operations.
    public var primary: ProviderKind
    /// Fallback provider used when primary is unavailable.
    public var fallback: ProviderKind
    /// Groq API key secret identifier.
    public var groqAPIKeyRef: String
    /// OpenAI API key secret identifier.
    public var openAIAPIKeyRef: String
    /// Request timeout for short clips in seconds.
    public var timeoutSeconds: Int
    /// Selected model identifier for Groq.
    public var groqModel: String
    /// Selected model identifier for OpenAI.
    public var openAIModel: String

    /// Creates provider configuration values.
    public init(
        primary: ProviderKind,
        fallback: ProviderKind,
        groqAPIKeyRef: String,
        openAIAPIKeyRef: String,
        timeoutSeconds: Int,
        groqModel: String,
        openAIModel: String
    ) {
        self.primary = primary
        self.fallback = fallback
        self.groqAPIKeyRef = groqAPIKeyRef
        self.openAIAPIKeyRef = openAIAPIKeyRef
        self.timeoutSeconds = timeoutSeconds
        self.groqModel = groqModel
        self.openAIModel = openAIModel
    }
}

/// Persisted application preferences.
public struct AppSettings: Codable, Sendable {
    /// Active distribution profile.
    public var buildProfile: BuildProfile
    /// Transcript output mode.
    public var outputMode: OutputMode
    /// Language code or `auto`.
    public var language: String
    /// Extra vocabulary hints.
    public var vocabularyHints: [String]
    /// Primary interaction mode.
    public var recordingInteraction: RecordingInteractionMode
    /// Declared hotkey mappings.
    public var hotkeys: [HotkeyBinding]
    /// Provider-specific settings.
    public var provider: ProviderConfiguration

    /// Creates settings data.
    public init(
        buildProfile: BuildProfile,
        outputMode: OutputMode,
        language: String,
        vocabularyHints: [String],
        recordingInteraction: RecordingInteractionMode,
        hotkeys: [HotkeyBinding],
        provider: ProviderConfiguration
    ) {
        self.buildProfile = buildProfile
        self.outputMode = outputMode
        self.language = language
        self.vocabularyHints = vocabularyHints
        self.recordingInteraction = recordingInteraction
        self.hotkeys = hotkeys
        self.provider = provider
    }

    /// Baseline defaults used for first launch.
    public static let `default` = AppSettings(
        buildProfile: .direct,
        outputMode: .clipboardAndPaste,
        language: "auto",
        vocabularyHints: [],
        recordingInteraction: .toggle,
        hotkeys: [
            HotkeyBinding(actionID: "toggle", keyCode: 18, modifiers: [.control, .shift]),
            HotkeyBinding(actionID: "retry", keyCode: 19, modifiers: [.control, .shift]),
            HotkeyBinding(actionID: "cancel", keyCode: 20, modifiers: [.control, .shift])
        ],
        provider: ProviderConfiguration(
            primary: .groq,
            fallback: .openAI,
            groqAPIKeyRef: "groq_api_key",
            openAIAPIKeyRef: "openai_api_key",
            timeoutSeconds: 12,
            groqModel: "whisper-large-v3",
            openAIModel: "gpt-4o-mini-transcribe"
        )
    )
}

/// A single settings validation error.
public struct SettingsValidationIssue: Error, Equatable, Sendable {
    /// Invalid field path.
    public let field: String
    /// Human-readable reason.
    public let message: String

    /// Creates a validation issue.
    public init(field: String, message: String) {
        self.field = field
        self.message = message
    }
}

/// Aggregated validation error container matching Python parity behavior.
public struct SettingsValidationErrorSet: Error, Sendable {
    /// All validation issues found in one pass.
    public let issues: [SettingsValidationIssue]

    /// Creates a grouped validation error.
    public init(issues: [SettingsValidationIssue]) {
        self.issues = issues
    }
}

/// Lifecycle-level degraded reasons.
public enum DegradedReason: String, Codable, Sendable {
    /// Missing required permissions.
    case permissions
    /// No active input device is available.
    case noInputDevice
    /// Provider health prevents transcription.
    case providerUnavailable
    /// Hotkey registration or dispatch failed.
    case hotkeyFailure
    /// Internal unexpected condition.
    case internalError
}

/// Session-level status used by history persistence.
public enum SessionStatus: String, Codable, Sendable {
    /// Session completed successfully.
    case success
    /// Session failed and can be retried.
    case retryAvailable
    /// Session was cancelled.
    case cancelled
    /// Session failed terminally.
    case failed
}

/// Captured session metadata and transcript output.
public struct SessionRecord: Sendable {
    /// Stable session UUID.
    public let sessionID: UUID
    /// Session creation timestamp.
    public let createdAt: Date
    /// Recorded audio duration in milliseconds.
    public let durationMS: Int
    /// Configured primary provider.
    public let providerPrimary: ProviderKind
    /// Provider used to generate final output.
    public let providerUsed: ProviderKind
    /// Language used for transcription.
    public let language: String
    /// Output mode used for routing.
    public let outputMode: OutputMode
    /// Final status.
    public let status: SessionStatus
    /// Transcript text.
    public let transcript: String
    /// Main audio file path.
    public let audioPath: URL

    /// Creates a persisted session value.
    public init(
        sessionID: UUID,
        createdAt: Date,
        durationMS: Int,
        providerPrimary: ProviderKind,
        providerUsed: ProviderKind,
        language: String,
        outputMode: OutputMode,
        status: SessionStatus,
        transcript: String,
        audioPath: URL
    ) {
        self.sessionID = sessionID
        self.createdAt = createdAt
        self.durationMS = durationMS
        self.providerPrimary = providerPrimary
        self.providerUsed = providerUsed
        self.language = language
        self.outputMode = outputMode
        self.status = status
        self.transcript = transcript
        self.audioPath = audioPath
    }
}
