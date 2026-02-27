import XCTest
@testable import QuedoCore

final class ConfigurationTests: XCTestCase {
    func testDefaultSettingsLoadWhenUnset() async throws {
        let suiteName = "ConfigurationTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Unable to create isolated UserDefaults suite")
            return
        }
        defaults.removePersistentDomain(forName: suiteName)

        let manager = ConfigurationManager(userDefaults: defaults, sharedConfigEnabled: false)
        let settings = try await manager.loadSettings()

        XCTAssertEqual(settings.outputMode, .clipboardAndPaste)
        XCTAssertEqual(settings.provider.primary, .groq)
    }

    func testValidationAggregatesErrors() async {
        let suiteName = "ConfigurationTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Unable to create isolated UserDefaults suite")
            return
        }
        defaults.removePersistentDomain(forName: suiteName)

        let manager = ConfigurationManager(userDefaults: defaults, sharedConfigEnabled: false)
        var invalid = AppSettings.default
        invalid.provider.timeoutSeconds = 0
        invalid.hotkeys = []
        invalid.provider.fallback = invalid.provider.primary

        do {
            try await manager.validate(settings: invalid)
            XCTFail("Expected aggregated validation failure")
        } catch let error as SettingsValidationErrorSet {
            XCTAssertGreaterThanOrEqual(error.issues.count, 2)
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    func testValidationAllowsNoHotkeys() async throws {
        let suiteName = "ConfigurationTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Unable to create isolated UserDefaults suite")
            return
        }
        defaults.removePersistentDomain(forName: suiteName)

        let manager = ConfigurationManager(userDefaults: defaults, sharedConfigEnabled: false)
        var settings = AppSettings.default
        settings.hotkeys = []

        try await manager.validate(settings: settings)
    }

    func testSaveAndReloadSettings() async throws {
        let suiteName = "ConfigurationTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Unable to create isolated UserDefaults suite")
            return
        }
        defaults.removePersistentDomain(forName: suiteName)

        let manager = ConfigurationManager(userDefaults: defaults, sharedConfigEnabled: false)
        var settings = AppSettings.default
        settings.language = "en"
        settings.outputMode = .clipboard

        try await manager.saveSettings(settings)
        let loaded = try await manager.loadSettings()

        XCTAssertEqual(loaded.language, "en")
        XCTAssertEqual(loaded.outputMode, .clipboard)
    }

    func testLegacyProviderConfigDecodesWithoutWhisperCppModelPath() throws {
        let legacyJSON = """
        {
          "provider": {
            "primary": "groq",
            "fallback": "openAI",
            "groqAPIKeyRef": "groq_api_key",
            "openAIAPIKeyRef": "openai_api_key",
            "timeoutSeconds": 12,
            "groqModel": "whisper-large-v3",
            "openAIModel": "gpt-4o-mini-transcribe"
          }
        }
        """

        let data = Data(legacyJSON.utf8)
        let decoded = try JSONDecoder().decode(AppSettings.self, from: data)

        XCTAssertEqual(decoded.provider.primary, .groq)
        XCTAssertEqual(decoded.provider.fallback, .openAI)
        XCTAssertEqual(decoded.provider.whisperCppModelPath, ProviderConfiguration.defaultValue.whisperCppModelPath)
    }
}
