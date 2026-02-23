import Foundation
import Security

/// Errors returned by configuration operations.
public enum ConfigurationError: Error, Sendable {
    /// Settings payload is missing or corrupted.
    case settingsNotFound
    /// Keychain operation failed.
    case keychainFailure(status: OSStatus)
}

/// Manages persisted settings and secrets with validation.
public actor ConfigurationManager {
    private enum Constants {
        static let userDefaultsKey = "whisper.assistant.settings.v1"
        static let keychainService = "com.whisperassistant.keys"
    }

    private let userDefaults: UserDefaults
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    /// Creates a configuration manager.
    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    /// Loads persisted settings or returns defaults when not yet configured.
    public func loadSettings() throws -> AppSettings {
        guard let data = userDefaults.data(forKey: Constants.userDefaultsKey) else {
            return .default
        }

        do {
            let settings = try decoder.decode(AppSettings.self, from: data)
            try validate(settings: settings)
            return settings
        } catch {
            throw ConfigurationError.settingsNotFound
        }
    }

    /// Validates and stores settings in UserDefaults.
    public func saveSettings(_ settings: AppSettings) throws {
        try validate(settings: settings)
        let data = try encoder.encode(settings)
        userDefaults.set(data, forKey: Constants.userDefaultsKey)
    }

    /// Stores provider API key in Keychain.
    public func saveAPIKey(_ key: String, for provider: ProviderKind) throws {
        let account = provider.rawValue
        let data = Data(key.utf8)

        let deleteQuery: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: Constants.keychainService,
            kSecAttrAccount: account
        ]
        SecItemDelete(deleteQuery as CFDictionary)

        let addQuery: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: Constants.keychainService,
            kSecAttrAccount: account,
            kSecValueData: data,
            kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlock
        ]

        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw ConfigurationError.keychainFailure(status: status)
        }
    }

    /// Reads provider API key from Keychain.
    public func loadAPIKey(for provider: ProviderKind) throws -> String? {
        let account = provider.rawValue
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: Constants.keychainService,
            kSecAttrAccount: account,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne
        ]

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)

        if status == errSecItemNotFound {
            return nil
        }

        guard status == errSecSuccess else {
            throw ConfigurationError.keychainFailure(status: status)
        }

        guard
            let data = item as? Data,
            let value = String(data: data, encoding: .utf8)
        else {
            return nil
        }

        return value
    }

    /// Validates all settings and throws aggregated failures.
    public func validate(settings: AppSettings) throws {
        var issues: [SettingsValidationIssue] = []

        if settings.provider.timeoutSeconds < 1 || settings.provider.timeoutSeconds > 120 {
            issues.append(SettingsValidationIssue(field: "provider.timeoutSeconds", message: "Timeout must be between 1 and 120 seconds"))
        }

        let validModes = Set(OutputMode.allCases)
        if !validModes.contains(settings.outputMode) {
            issues.append(SettingsValidationIssue(field: "outputMode", message: "Unsupported output mode"))
        }

        if settings.provider.primary == settings.provider.fallback {
            issues.append(SettingsValidationIssue(field: "provider.fallback", message: "Primary and fallback provider must differ"))
        }

        let normalizedLanguage = settings.language.trimmingCharacters(in: .whitespacesAndNewlines)
        if normalizedLanguage.isEmpty {
            issues.append(SettingsValidationIssue(field: "language", message: "Language cannot be empty"))
        }

        var seenActions = Set<String>()
        for hotkey in settings.hotkeys {
            if seenActions.contains(hotkey.actionID) {
                issues.append(SettingsValidationIssue(field: "hotkeys", message: "Duplicate actionID \(hotkey.actionID)"))
            }
            seenActions.insert(hotkey.actionID)

            if hotkey.modifiers.isEmpty {
                issues.append(SettingsValidationIssue(field: "hotkeys.\(hotkey.actionID)", message: "At least one modifier is required"))
            }
        }

        if !seenActions.contains("toggle") {
            issues.append(SettingsValidationIssue(field: "hotkeys", message: "Missing required toggle hotkey"))
        }

        if settings.provider.groqModel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            issues.append(SettingsValidationIssue(field: "provider.groqModel", message: "Groq model must not be empty"))
        }

        if settings.provider.openAIModel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            issues.append(SettingsValidationIssue(field: "provider.openAIModel", message: "OpenAI model must not be empty"))
        }

        if issues.isEmpty {
            return
        }

        throw SettingsValidationErrorSet(issues: issues)
    }

    /// Produces sanitized settings snapshot for diagnostics and CLI output.
    public func redactedSnapshot() throws -> [String: String] {
        let settings = try loadSettings()
        return [
            "buildProfile": settings.buildProfile.rawValue,
            "outputMode": settings.outputMode.rawValue,
            "language": settings.language,
            "recordingInteraction": settings.recordingInteraction.rawValue,
            "primaryProvider": settings.provider.primary.rawValue,
            "fallbackProvider": settings.provider.fallback.rawValue,
            "timeoutSeconds": String(settings.provider.timeoutSeconds),
            "groqModel": settings.provider.groqModel,
            "openAIModel": settings.provider.openAIModel,
            "vocabularyHintsCount": String(settings.vocabularyHints.count)
        ]
    }
}
