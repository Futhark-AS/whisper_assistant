import AppKit
import Carbon
import SwiftUI
import QuedoCore

/// Preset hotkey layouts exposed in preferences.
enum HotkeyPreset: String, CaseIterable, Identifiable {
    case fnControl = "Fn + Ctrl (Recommended)"
    case ctrlShift = "Ctrl + Shift (Legacy)"
    case manual = "Manual (Custom Strings)"

    var id: String { rawValue }
}

/// Preferences window for application configuration.
struct PreferencesView: View {
    @StateObject private var model: PreferencesViewModel

    init(
        configurationManager: ConfigurationManager,
        onSaved: @escaping @MainActor @Sendable () -> Void = {}
    ) {
        _model = StateObject(
            wrappedValue: PreferencesViewModel(
                configurationManager: configurationManager,
                onSaved: onSaved
            )
        )
    }

    var body: some View {
        VStack(spacing: 12) {
            Form {
                Section("Behavior") {
                    Picker("Interaction", selection: $model.recordingInteraction) {
                        ForEach(RecordingInteractionMode.allCases, id: \.self) { mode in
                            Text(mode.rawValue.capitalized).tag(mode)
                        }
                    }
                    .pickerStyle(.menu)

                    Toggle("Launch at login", isOn: $model.launchAtLoginEnabled)

                    Picker("Output", selection: $model.outputMode) {
                        ForEach(OutputMode.allCases, id: \.self) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                    .pickerStyle(.menu)

                    Text("Choose `clipboard` if you never want auto-paste.")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TextField("Language (`auto`, `en`, `no`...)", text: $model.language)
                    TextField("Vocabulary hints (comma separated)", text: $model.vocabularyText)
                }

                Section("Providers") {
                    Picker("Primary Provider", selection: $model.primaryProvider) {
                        ForEach(ProviderKind.allCases, id: \.self) { provider in
                            Text(provider.rawValue).tag(provider)
                        }
                    }
                    .pickerStyle(.menu)

                    Picker("Fallback Provider", selection: $model.fallbackProvider) {
                        ForEach(ProviderKind.allCases, id: \.self) { provider in
                            Text(provider.rawValue).tag(provider)
                        }
                    }
                    .pickerStyle(.menu)

                    Stepper(
                        "Timeout: \(model.timeoutSeconds)s",
                        value: $model.timeoutSeconds,
                        in: 1...120
                    )

                    TextField("Groq Model", text: $model.groqModel)
                    TextField("OpenAI Model", text: $model.openAIModel)
                }

                Section("API Keys") {
                    HStack(spacing: 8) {
                        SecureField("Groq API key (leave blank to keep current)", text: $model.groqAPIKeyInput)
                        Button("Paste") {
                            model.pasteAPIKey(.groq)
                        }
                    }
                    Text(model.hasGroqKey ? "Groq key: stored" : "Groq key: missing")
                        .font(.caption)
                        .foregroundColor(model.hasGroqKey ? .secondary : .orange)

                    HStack(spacing: 8) {
                        SecureField("OpenAI API key (optional)", text: $model.openAIAPIKeyInput)
                        Button("Paste") {
                            model.pasteAPIKey(.openAI)
                        }
                    }
                    Text(model.hasOpenAIKey ? "OpenAI key: stored" : "OpenAI key: missing")
                        .font(.caption)
                        .foregroundColor(model.hasOpenAIKey ? .secondary : .orange)
                }

                Section("Hotkeys") {
                    Toggle("Enable global hotkeys", isOn: $model.hotkeysEnabled)

                    if model.hotkeysEnabled {
                        Picker("Shortcut Mode", selection: $model.hotkeyPreset) {
                            ForEach(model.availableHotkeyPresets, id: \.self) { preset in
                                Text(preset.rawValue).tag(preset)
                            }
                        }
                        .pickerStyle(.menu)

                        if model.hotkeyPreset == .manual {
                            HStack(spacing: 8) {
                                TextField("Toggle recording (`fn+ctrl`, `cmd+shift+r`...)", text: $model.manualToggleHotkeyText)
                                Button("Paste") {
                                    model.pasteManualHotkey(.toggle)
                                }
                            }
                            HStack(spacing: 8) {
                                TextField("Retry transcription", text: $model.manualRetryHotkeyText)
                                Button("Paste") {
                                    model.pasteManualHotkey(.retry)
                                }
                            }
                            HStack(spacing: 8) {
                                TextField("Cancel recording", text: $model.manualCancelHotkeyText)
                                Button("Paste") {
                                    model.pasteManualHotkey(.cancel)
                                }
                            }

                            Text("Manual format: modifiers + key (example `cmd+shift+r`) or modifiers-only (`fn+ctrl`). Leave field empty to unbind that action.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }

                        Text(model.hotkeySummary)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        Text("Hotkeys disabled. Use menu bar actions only.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .formStyle(.grouped)

            Divider()

            HStack(spacing: 10) {
                Button("Save") {
                    Task { await model.save() }
                }
                .keyboardShortcut(.defaultAction)

                Button("Restore Defaults") {
                    model.applyDefaults()
                }

                Spacer()

                if let status = model.statusMessage {
                    Text(status)
                        .font(.caption)
                        .foregroundStyle(model.statusIsError ? .red : .secondary)
                }
            }
        }
        .padding(16)
        .frame(width: 680, height: 760)
        .task {
            await model.load()
        }
    }
}

@MainActor
final class PreferencesViewModel: ObservableObject {
    @Published var outputMode: OutputMode = .clipboardAndPaste
    @Published var recordingInteraction: RecordingInteractionMode = .toggle
    @Published var launchAtLoginEnabled = true
    @Published var language = "auto"
    @Published var vocabularyText = ""
    @Published var primaryProvider: ProviderKind = .groq
    @Published var fallbackProvider: ProviderKind = .openAI
    @Published var timeoutSeconds = 12
    @Published var groqModel = "whisper-large-v3"
    @Published var openAIModel = "gpt-4o-mini-transcribe"

    @Published var hotkeysEnabled = true
    @Published var hotkeyPreset: HotkeyPreset = .fnControl
    @Published var manualToggleHotkeyText = "fn+ctrl"
    @Published var manualRetryHotkeyText = "fn+ctrl+r"
    @Published var manualCancelHotkeyText = "fn+ctrl+escape"

    @Published var groqAPIKeyInput = ""
    @Published var openAIAPIKeyInput = ""
    @Published var hasGroqKey = false
    @Published var hasOpenAIKey = false

    @Published var statusMessage: String?
    @Published var statusIsError = false

    private let configurationManager: ConfigurationManager
    private let onSaved: @MainActor @Sendable () -> Void
    private var loadedSettings = AppSettings.default

    init(
        configurationManager: ConfigurationManager,
        onSaved: @escaping @MainActor @Sendable () -> Void
    ) {
        self.configurationManager = configurationManager
        self.onSaved = onSaved
    }

    var availableHotkeyPresets: [HotkeyPreset] {
        HotkeyPreset.allCases
    }

    var hotkeySummary: String {
        do {
            let bindings = try hotkeysForSave()
            guard !bindings.isEmpty else {
                return "No shortcuts configured."
            }
            return bindings.map(describeHotkey).joined(separator: "  |  ")
        } catch let error as SettingsValidationErrorSet {
            return error.issues.map { "\($0.field): \($0.message)" }.joined(separator: " | ")
        } catch {
            return "Invalid hotkey configuration."
        }
    }

    enum ManualHotkeyField {
        case toggle
        case retry
        case cancel
    }

    func pasteAPIKey(_ provider: ProviderKind) {
        guard let value = clipboardString(), !value.isEmpty else {
            return
        }

        switch provider {
        case .groq:
            groqAPIKeyInput = value
        case .openAI:
            openAIAPIKeyInput = value
        }
    }

    func pasteManualHotkey(_ field: ManualHotkeyField) {
        guard let value = clipboardString(), !value.isEmpty else {
            return
        }

        switch field {
        case .toggle:
            manualToggleHotkeyText = value
        case .retry:
            manualRetryHotkeyText = value
        case .cancel:
            manualCancelHotkeyText = value
        }
    }

    func load() async {
        do {
            let settings = try await configurationManager.loadSettings()
            loadedSettings = settings

            outputMode = settings.outputMode
            recordingInteraction = settings.recordingInteraction
            launchAtLoginEnabled = settings.launchAtLoginEnabled
            language = settings.language
            vocabularyText = settings.vocabularyHints.joined(separator: ", ")
            primaryProvider = settings.provider.primary
            fallbackProvider = settings.provider.fallback
            timeoutSeconds = settings.provider.timeoutSeconds
            groqModel = settings.provider.groqModel
            openAIModel = settings.provider.openAIModel

            hotkeysEnabled = !settings.hotkeys.isEmpty
            hotkeyPreset = detectPreset(hotkeys: settings.hotkeys)
            loadManualHotkeys(from: settings.hotkeys)

            hasGroqKey = (try await configurationManager.loadAPIKey(for: .groq)) != nil
            hasOpenAIKey = (try await configurationManager.loadAPIKey(for: .openAI)) != nil

            statusMessage = nil
            statusIsError = false
        } catch {
            statusMessage = "Failed to load settings"
            statusIsError = true
        }
    }

    func applyDefaults() {
        let defaults = AppSettings.default
        outputMode = defaults.outputMode
        recordingInteraction = defaults.recordingInteraction
        launchAtLoginEnabled = defaults.launchAtLoginEnabled
        language = defaults.language
        vocabularyText = defaults.vocabularyHints.joined(separator: ", ")
        primaryProvider = defaults.provider.primary
        fallbackProvider = defaults.provider.fallback
        timeoutSeconds = defaults.provider.timeoutSeconds
        groqModel = defaults.provider.groqModel
        openAIModel = defaults.provider.openAIModel

        hotkeysEnabled = true
        hotkeyPreset = .fnControl
        loadManualHotkeys(from: defaults.hotkeys)

        statusMessage = "Defaults loaded. Save to apply."
        statusIsError = false
    }

    func save() async {
        loadedSettings.outputMode = outputMode
        loadedSettings.recordingInteraction = recordingInteraction
        loadedSettings.launchAtLoginEnabled = launchAtLoginEnabled
        loadedSettings.language = language.trimmingCharacters(in: .whitespacesAndNewlines)
        loadedSettings.vocabularyHints = vocabularyText
            .split(separator: ",")
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        loadedSettings.provider.primary = primaryProvider
        loadedSettings.provider.fallback = fallbackProvider
        loadedSettings.provider.timeoutSeconds = timeoutSeconds
        loadedSettings.provider.groqModel = groqModel.trimmingCharacters(in: .whitespacesAndNewlines)
        loadedSettings.provider.openAIModel = openAIModel.trimmingCharacters(in: .whitespacesAndNewlines)

        do {
            loadedSettings.hotkeys = try hotkeysForSave()
            try await configurationManager.saveSettings(loadedSettings)

            let groqTrimmed = groqAPIKeyInput.trimmingCharacters(in: .whitespacesAndNewlines)
            if !groqTrimmed.isEmpty {
                try await configurationManager.saveAPIKey(groqTrimmed, for: .groq)
                groqAPIKeyInput = ""
            }

            let openAITrimmed = openAIAPIKeyInput.trimmingCharacters(in: .whitespacesAndNewlines)
            if !openAITrimmed.isEmpty {
                try await configurationManager.saveAPIKey(openAITrimmed, for: .openAI)
                openAIAPIKeyInput = ""
            }

            hasGroqKey = (try await configurationManager.loadAPIKey(for: .groq)) != nil
            hasOpenAIKey = (try await configurationManager.loadAPIKey(for: .openAI)) != nil

            statusMessage = "Saved and applied."
            statusIsError = false
            onSaved()
        } catch let error as SettingsValidationErrorSet {
            statusMessage = error.issues.map { "\($0.field): \($0.message)" }.joined(separator: " | ")
            statusIsError = true
        } catch {
            statusMessage = "Failed to save settings: \(error.localizedDescription)"
            statusIsError = true
        }
    }

    private func hotkeysForSave() throws -> [HotkeyBinding] {
        guard hotkeysEnabled else {
            return []
        }

        switch hotkeyPreset {
        case .fnControl:
            return [
                HotkeyBinding(actionID: "toggle", keyCode: HotkeyBinding.modifiersOnlyKeyCode, modifiers: [.control, .function]),
                HotkeyBinding(actionID: "retry", keyCode: UInt32(kVK_ANSI_R), modifiers: [.control, .function]),
                HotkeyBinding(actionID: "cancel", keyCode: UInt32(kVK_Escape), modifiers: [.control, .function])
            ]
        case .ctrlShift:
            return [
                HotkeyBinding(actionID: "toggle", keyCode: UInt32(kVK_ANSI_1), modifiers: [.control, .shift]),
                HotkeyBinding(actionID: "retry", keyCode: UInt32(kVK_ANSI_2), modifiers: [.control, .shift]),
                HotkeyBinding(actionID: "cancel", keyCode: UInt32(kVK_ANSI_3), modifiers: [.control, .shift])
            ]
        case .manual:
            return try parseManualHotkeys()
        }
    }

    private func parseManualHotkeys() throws -> [HotkeyBinding] {
        var bindings: [HotkeyBinding] = []
        var issues: [SettingsValidationIssue] = []

        let rows: [(actionID: String, field: String, rawValue: String)] = [
            ("toggle", "hotkeys.toggle", manualToggleHotkeyText),
            ("retry", "hotkeys.retry", manualRetryHotkeyText),
            ("cancel", "hotkeys.cancel", manualCancelHotkeyText)
        ]

        for row in rows {
            let trimmed = row.rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                continue
            }

            guard let parsed = HotkeyCodec.parse(trimmed, actionID: row.actionID) else {
                issues.append(
                    SettingsValidationIssue(
                        field: row.field,
                        message: "Invalid format. Example: cmd+shift+r or fn+ctrl"
                    )
                )
                continue
            }

            bindings.append(parsed)
        }

        if !issues.isEmpty {
            throw SettingsValidationErrorSet(issues: issues)
        }

        return bindings
    }

    private func detectPreset(hotkeys: [HotkeyBinding]) -> HotkeyPreset {
        let fnControl: [HotkeyBinding] = [
            HotkeyBinding(actionID: "toggle", keyCode: HotkeyBinding.modifiersOnlyKeyCode, modifiers: [.control, .function]),
            HotkeyBinding(actionID: "retry", keyCode: UInt32(kVK_ANSI_R), modifiers: [.control, .function]),
            HotkeyBinding(actionID: "cancel", keyCode: UInt32(kVK_Escape), modifiers: [.control, .function])
        ]
        let ctrlShift: [HotkeyBinding] = [
            HotkeyBinding(actionID: "toggle", keyCode: UInt32(kVK_ANSI_1), modifiers: [.control, .shift]),
            HotkeyBinding(actionID: "retry", keyCode: UInt32(kVK_ANSI_2), modifiers: [.control, .shift]),
            HotkeyBinding(actionID: "cancel", keyCode: UInt32(kVK_ANSI_3), modifiers: [.control, .shift])
        ]

        if hotkeys == fnControl {
            return .fnControl
        }
        if hotkeys == ctrlShift {
            return .ctrlShift
        }
        return .manual
    }

    private func loadManualHotkeys(from hotkeys: [HotkeyBinding]) {
        let byAction = Dictionary(uniqueKeysWithValues: hotkeys.map { ($0.actionID, $0) })
        manualToggleHotkeyText = byAction["toggle"].flatMap(HotkeyCodec.render) ?? ""
        manualRetryHotkeyText = byAction["retry"].flatMap(HotkeyCodec.render) ?? ""
        manualCancelHotkeyText = byAction["cancel"].flatMap(HotkeyCodec.render) ?? ""
    }

    private func describeHotkey(_ binding: HotkeyBinding) -> String {
        let actionName: String
        switch binding.actionID {
        case "toggle": actionName = "Toggle"
        case "retry": actionName = "Retry"
        case "cancel": actionName = "Cancel"
        default: actionName = binding.actionID
        }
        return "\(actionName): \(HotkeyCodec.displayString(binding))"
    }

    private func clipboardString() -> String? {
        NSPasteboard.general.string(forType: .string)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
