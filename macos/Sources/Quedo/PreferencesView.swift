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

private enum PreferencesPane: String, CaseIterable, Identifiable {
    case general
    case providers
    case apiKeys
    case hotkeys

    var id: String { rawValue }

    var title: String {
        switch self {
        case .general: return "General"
        case .providers: return "Providers"
        case .apiKeys: return "API Keys"
        case .hotkeys: return "Hotkeys"
        }
    }

    var subtitle: String {
        switch self {
        case .general:
            return "Language, output, and launch behavior."
        case .providers:
            return "Routing, timeout, and model selection."
        case .apiKeys:
            return "Manage provider credentials."
        case .hotkeys:
            return "Global shortcuts and manual bindings."
        }
    }

    var symbol: String {
        switch self {
        case .general: return "slider.horizontal.3"
        case .providers: return "network"
        case .apiKeys: return "key.horizontal"
        case .hotkeys: return "keyboard"
        }
    }
}

private struct PreferencesCard<Content: View>: View {
    let title: String
    let subtitle: String?
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.headline)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            content
        }
        .padding(16)
        .background {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor).opacity(0.65))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(Color.secondary.opacity(0.18), lineWidth: 1)
        }
    }
}

/// Preferences window for application configuration.
struct PreferencesView: View {
    @StateObject private var model: PreferencesViewModel
    @State private var selectedPane: PreferencesPane = .general

    private let rowLabelWidth: CGFloat = 130

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
        NavigationSplitView {
            List(PreferencesPane.allCases, selection: $selectedPane) { pane in
                Label(pane.title, systemImage: pane.symbol)
                    .tag(pane)
            }
            .listStyle(.sidebar)
            .navigationTitle("Preferences")
            .navigationSplitViewColumnWidth(min: 180, ideal: 220, max: 260)
        } detail: {
            VStack(spacing: 0) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(selectedPane.title)
                        .font(.title2.weight(.semibold))
                    Text(selectedPane.subtitle)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20)
                .padding(.top, 18)
                .padding(.bottom, 14)

                Divider()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        paneContent(for: selectedPane)
                    }
                    .padding(20)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                }

                Divider()

                HStack(spacing: 10) {
                    Button("Restore Defaults") {
                        model.applyDefaults()
                    }
                    .disabled(model.isSaving)

                    Button("Discard Changes") {
                        model.discardChanges()
                    }
                    .disabled(!model.hasUnsavedChanges || model.isSaving)

                    Spacer()

                    if model.hasUnsavedChanges {
                        Label("Unsaved changes", systemImage: "circle.fill")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }

                    if let status = model.statusMessage {
                        Text(status)
                            .font(.caption)
                            .foregroundStyle(model.statusIsError ? .red : .secondary)
                            .lineLimit(2)
                    }

                    Button(model.isSaving ? "Saving..." : "Save Changes") {
                        Task { await model.save() }
                    }
                    .keyboardShortcut("s", modifiers: [.command])
                    .buttonStyle(.borderedProminent)
                    .disabled(!model.canSave)
                }
                .padding(16)
            }
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .frame(minWidth: 880, minHeight: 680)
        .task {
            await model.load()
        }
        .animation(.easeInOut(duration: 0.15), value: selectedPane)
    }

    @ViewBuilder
    private func paneContent(for pane: PreferencesPane) -> some View {
        switch pane {
        case .general:
            generalPane
        case .providers:
            providersPane
        case .apiKeys:
            apiKeysPane
        case .hotkeys:
            hotkeysPane
        }
    }

    private var generalPane: some View {
        VStack(alignment: .leading, spacing: 16) {
            PreferencesCard(
                title: "Behavior",
                subtitle: "Core app interaction and output delivery."
            ) {
                settingRow("Interaction") {
                    Picker("", selection: $model.recordingInteraction) {
                        Text("Toggle").tag(RecordingInteractionMode.toggle)
                        Text("Hold").tag(RecordingInteractionMode.hold)
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .frame(maxWidth: 220)
                }

                settingRow("Output") {
                    Picker("", selection: $model.outputMode) {
                        ForEach(OutputMode.allCases, id: \.self) { mode in
                            Text(outputLabel(mode)).tag(mode)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .frame(maxWidth: 260)
                }

                settingRow("Launch at login") {
                    Toggle("", isOn: $model.launchAtLoginEnabled)
                        .labelsHidden()
                }

                settingRow("Language") {
                    TextField("auto, en, no...", text: $model.language)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 240)
                }
            }

            PreferencesCard(
                title: "Vocabulary Hints",
                subtitle: "Paste one term per line, or comma-separated."
            ) {
                TextEditor(text: $model.vocabularyText)
                    .font(.body)
                    .padding(8)
                    .frame(minHeight: 140)
                    .background(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(Color(nsColor: .textBackgroundColor))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
                    )

                HStack(spacing: 8) {
                    Button("Paste from Clipboard") {
                        model.pasteVocabulary()
                    }
                    .buttonStyle(.bordered)

                    Button("Clear") {
                        model.clearVocabulary()
                    }
                    .buttonStyle(.bordered)

                    Spacer()

                    Text("\(model.vocabularyHintCount) hints")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var providersPane: some View {
        VStack(alignment: .leading, spacing: 16) {
            PreferencesCard(
                title: "Provider Routing",
                subtitle: "Primary handles normal requests; fallback handles failures."
            ) {
                settingRow("Primary provider") {
                    Picker("", selection: $model.primaryProvider) {
                        ForEach(ProviderKind.allCases, id: \.self) { provider in
                            Text(providerLabel(provider)).tag(provider)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .frame(maxWidth: 220)
                }

                settingRow("Fallback provider") {
                    Picker("", selection: $model.fallbackProvider) {
                        ForEach(ProviderKind.allCases, id: \.self) { provider in
                            Text(providerLabel(provider)).tag(provider)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .frame(maxWidth: 220)
                }

                if let providerError = model.providerSelectionError {
                    Label(providerError, systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .padding(.leading, rowLabelWidth + 12)
                }
            }

            PreferencesCard(
                title: "Models and Timeout",
                subtitle: "Tune model names and request timeout."
            ) {
                settingRow("Timeout") {
                    Stepper(value: $model.timeoutSeconds, in: 1...120) {
                        Text("\(model.timeoutSeconds) seconds")
                    }
                    .frame(maxWidth: 220, alignment: .leading)
                }

                settingRow("Groq model") {
                    TextField("whisper-large-v3", text: $model.groqModel)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(.body, design: .monospaced))
                        .frame(maxWidth: 340)
                }

                settingRow("OpenAI model") {
                    TextField("gpt-4o-mini-transcribe", text: $model.openAIModel)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(.body, design: .monospaced))
                        .frame(maxWidth: 340)
                }
            }
        }
    }

    private var apiKeysPane: some View {
        VStack(alignment: .leading, spacing: 16) {
            PreferencesCard(
                title: "Groq",
                subtitle: "Used when Groq is primary or fallback."
            ) {
                settingRow("API key") {
                    HStack(spacing: 8) {
                        SecureField("Paste new key (optional)", text: $model.groqAPIKeyInput)
                            .textFieldStyle(.roundedBorder)
                            .frame(maxWidth: 400)

                        Label(
                            model.hasGroqKey ? "Stored" : "Missing",
                            systemImage: model.hasGroqKey ? "checkmark.circle.fill" : "exclamationmark.triangle.fill"
                        )
                        .font(.callout)
                        .foregroundStyle(model.hasGroqKey ? Color.accentColor : Color.orange)
                    }
                }

                HStack(spacing: 8) {
                    Button("Paste") {
                        model.pasteAPIKey(.groq)
                    }
                    .buttonStyle(.bordered)

                    Button("Clear Input") {
                        model.clearAPIKeyInput(.groq)
                    }
                    .buttonStyle(.bordered)
                    .disabled(model.groqAPIKeyInput.isEmpty)

                    Spacer()

                    Button("Remove Stored Key", role: .destructive) {
                        Task { await model.clearStoredAPIKey(.groq) }
                    }
                    .buttonStyle(.bordered)
                    .disabled(!model.hasGroqKey || model.isSaving)
                }
            }

            PreferencesCard(
                title: "OpenAI",
                subtitle: "Optional unless OpenAI is selected."
            ) {
                settingRow("API key") {
                    HStack(spacing: 8) {
                        SecureField("Paste new key (optional)", text: $model.openAIAPIKeyInput)
                            .textFieldStyle(.roundedBorder)
                            .frame(maxWidth: 400)

                        Label(
                            model.hasOpenAIKey ? "Stored" : "Missing",
                            systemImage: model.hasOpenAIKey ? "checkmark.circle.fill" : "exclamationmark.triangle.fill"
                        )
                        .font(.callout)
                        .foregroundStyle(model.hasOpenAIKey ? Color.accentColor : Color.orange)
                    }
                }

                HStack(spacing: 8) {
                    Button("Paste") {
                        model.pasteAPIKey(.openAI)
                    }
                    .buttonStyle(.bordered)

                    Button("Clear Input") {
                        model.clearAPIKeyInput(.openAI)
                    }
                    .buttonStyle(.bordered)
                    .disabled(model.openAIAPIKeyInput.isEmpty)

                    Spacer()

                    Button("Remove Stored Key", role: .destructive) {
                        Task { await model.clearStoredAPIKey(.openAI) }
                    }
                    .buttonStyle(.bordered)
                    .disabled(!model.hasOpenAIKey || model.isSaving)
                }
            }
        }
    }

    private var hotkeysPane: some View {
        VStack(alignment: .leading, spacing: 16) {
            PreferencesCard(
                title: "Global Hotkeys",
                subtitle: "Shortcut profile used for recording and controls."
            ) {
                settingRow("Enable hotkeys") {
                    Toggle("", isOn: $model.hotkeysEnabled)
                        .labelsHidden()
                }

                if model.hotkeysEnabled {
                    settingRow("Shortcut mode") {
                        Picker("", selection: $model.hotkeyPreset) {
                            ForEach(model.availableHotkeyPresets, id: \.self) { preset in
                                Text(preset.rawValue).tag(preset)
                            }
                        }
                        .labelsHidden()
                        .pickerStyle(.menu)
                        .frame(maxWidth: 280)
                    }

                    Text(model.hotkeySummary)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .padding(.leading, rowLabelWidth + 12)
                } else {
                    Text("Hotkeys disabled. Use menu bar actions only.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .padding(.leading, rowLabelWidth + 12)
                }
            }

            if model.hotkeysEnabled, model.hotkeyPreset == .manual {
                PreferencesCard(
                    title: "Manual Shortcuts",
                    subtitle: "Format: cmd+shift+r or fn+ctrl. Leave empty to unbind."
                ) {
                    manualHotkeyEditor(
                        title: "Toggle recording",
                        placeholder: "fn+ctrl",
                        field: .toggle,
                        text: $model.manualToggleHotkeyText
                    )

                    manualHotkeyEditor(
                        title: "Retry transcription",
                        placeholder: "fn+ctrl+r",
                        field: .retry,
                        text: $model.manualRetryHotkeyText
                    )

                    manualHotkeyEditor(
                        title: "Cancel recording",
                        placeholder: "fn+ctrl+escape",
                        field: .cancel,
                        text: $model.manualCancelHotkeyText
                    )
                }
            }
        }
    }

    @ViewBuilder
    private func settingRow<Content: View>(
        _ title: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        HStack(alignment: .center, spacing: 12) {
            Text(title)
                .foregroundStyle(.secondary)
                .frame(width: rowLabelWidth, alignment: .leading)

            content()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private func manualHotkeyEditor(
        title: String,
        placeholder: String,
        field: PreferencesViewModel.ManualHotkeyField,
        text: Binding<String>
    ) -> some View {
        settingRow(title) {
            HStack(spacing: 8) {
                TextField(placeholder, text: text)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: 320)

                Button("Paste") {
                    model.pasteManualHotkey(field)
                }
                .buttonStyle(.bordered)
            }
        }

        if let error = model.manualHotkeyError(for: field) {
            Text(error)
                .font(.caption)
                .foregroundStyle(.red)
                .padding(.leading, rowLabelWidth + 12)
        }
    }

    private func outputLabel(_ mode: OutputMode) -> String {
        switch mode {
        case .none:
            return "None"
        case .clipboard:
            return "Clipboard"
        case .pasteAtCursor:
            return "Paste at Cursor"
        case .clipboardAndPaste:
            return "Clipboard + Paste"
        }
    }

    private func providerLabel(_ provider: ProviderKind) -> String {
        switch provider {
        case .groq:
            return "Groq"
        case .openAI:
            return "OpenAI"
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
    @Published var isSaving = false

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

    var vocabularyHintCount: Int {
        parseVocabularyHints(vocabularyText).count
    }

    var providerSelectionError: String? {
        if primaryProvider == fallbackProvider {
            return "Primary and fallback providers must be different."
        }
        return nil
    }

    var hasInlineValidationErrors: Bool {
        if providerSelectionError != nil {
            return true
        }
        guard hotkeysEnabled, hotkeyPreset == .manual else {
            return false
        }
        return manualHotkeyError(for: .toggle) != nil
            || manualHotkeyError(for: .retry) != nil
            || manualHotkeyError(for: .cancel) != nil
    }

    var hasUnsavedChanges: Bool {
        let current = currentSnapshot()
        let loaded = snapshot(from: loadedSettings)
        let hasPendingAPIInput = !normalize(groqAPIKeyInput).isEmpty || !normalize(openAIAPIKeyInput).isEmpty
        return current != loaded || hasPendingAPIInput
    }

    var canSave: Bool {
        hasUnsavedChanges && !isSaving && !hasInlineValidationErrors
    }

    enum ManualHotkeyField {
        case toggle
        case retry
        case cancel

        var actionID: String {
            switch self {
            case .toggle: return "toggle"
            case .retry: return "retry"
            case .cancel: return "cancel"
            }
        }

        var fieldPath: String {
            switch self {
            case .toggle: return "hotkeys.toggle"
            case .retry: return "hotkeys.retry"
            case .cancel: return "hotkeys.cancel"
            }
        }
    }

    func clearVocabulary() {
        vocabularyText = ""
    }

    func clearAPIKeyInput(_ provider: ProviderKind) {
        switch provider {
        case .groq:
            groqAPIKeyInput = ""
        case .openAI:
            openAIAPIKeyInput = ""
        }
    }

    func manualHotkeyError(for field: ManualHotkeyField) -> String? {
        let rawValue: String
        switch field {
        case .toggle:
            rawValue = manualToggleHotkeyText
        case .retry:
            rawValue = manualRetryHotkeyText
        case .cancel:
            rawValue = manualCancelHotkeyText
        }

        let trimmed = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }

        guard HotkeyCodec.parse(trimmed, actionID: field.actionID) != nil else {
            return "Invalid format. Example: cmd+shift+r or fn+ctrl"
        }
        return nil
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

    func pasteVocabulary() {
        guard let value = clipboardString(), !value.isEmpty else {
            return
        }

        if vocabularyText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            vocabularyText = value
            return
        }
        vocabularyText += "\n\(value)"
    }

    func clearStoredAPIKey(_ provider: ProviderKind) async {
        do {
            try await configurationManager.clearAPIKey(for: provider)
            switch provider {
            case .groq:
                hasGroqKey = false
                groqAPIKeyInput = ""
                statusMessage = "Groq key removed."
            case .openAI:
                hasOpenAIKey = false
                openAIAPIKeyInput = ""
                statusMessage = "OpenAI key removed."
            }
            statusIsError = false
        } catch {
            statusMessage = "Failed to remove stored key: \(error.localizedDescription)"
            statusIsError = true
        }
    }

    func load() async {
        do {
            let settings = try await configurationManager.loadSettings()
            loadedSettings = settings
            apply(settings: settings)

            groqAPIKeyInput = ""
            openAIAPIKeyInput = ""
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
        apply(settings: .default)
        groqAPIKeyInput = ""
        openAIAPIKeyInput = ""
        statusMessage = "Defaults loaded. Save to apply."
        statusIsError = false
    }

    func discardChanges() {
        apply(settings: loadedSettings)
        groqAPIKeyInput = ""
        openAIAPIKeyInput = ""
        statusMessage = "Changes discarded."
        statusIsError = false
    }

    func save() async {
        guard !isSaving else {
            return
        }

        if !hasUnsavedChanges {
            statusMessage = "No changes to save."
            statusIsError = false
            return
        }

        if let providerSelectionError {
            statusMessage = providerSelectionError
            statusIsError = true
            return
        }

        if hotkeysEnabled, hotkeyPreset == .manual {
            if let hotkeyError = manualHotkeyError(for: .toggle)
                ?? manualHotkeyError(for: .retry)
                ?? manualHotkeyError(for: .cancel)
            {
                statusMessage = hotkeyError
                statusIsError = true
                return
            }
        }

        isSaving = true
        defer { isSaving = false }

        loadedSettings.outputMode = outputMode
        loadedSettings.recordingInteraction = recordingInteraction
        loadedSettings.launchAtLoginEnabled = launchAtLoginEnabled
        loadedSettings.language = normalize(language)
        loadedSettings.vocabularyHints = parseVocabularyHints(vocabularyText)

        loadedSettings.provider.primary = primaryProvider
        loadedSettings.provider.fallback = fallbackProvider
        loadedSettings.provider.timeoutSeconds = timeoutSeconds
        loadedSettings.provider.groqModel = normalize(groqModel)
        loadedSettings.provider.openAIModel = normalize(openAIModel)

        do {
            loadedSettings.hotkeys = try hotkeysForSave()
            try await configurationManager.saveSettings(loadedSettings)

            let groqTrimmed = normalize(groqAPIKeyInput)
            if !groqTrimmed.isEmpty {
                try await configurationManager.saveAPIKey(groqTrimmed, for: .groq)
                groqAPIKeyInput = ""
            }

            let openAITrimmed = normalize(openAIAPIKeyInput)
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

        let rows: [(field: ManualHotkeyField, rawValue: String)] = [
            (.toggle, manualToggleHotkeyText),
            (.retry, manualRetryHotkeyText),
            (.cancel, manualCancelHotkeyText)
        ]

        for row in rows {
            let trimmed = row.rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                continue
            }

            guard let parsed = HotkeyCodec.parse(trimmed, actionID: row.field.actionID) else {
                issues.append(
                    SettingsValidationIssue(
                        field: row.field.fieldPath,
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

    private func parseVocabularyHints(_ raw: String) -> [String] {
        raw
            .split(whereSeparator: { $0 == "," || $0 == "\n" })
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
    }

    private func apply(settings: AppSettings) {
        outputMode = settings.outputMode
        recordingInteraction = settings.recordingInteraction
        launchAtLoginEnabled = settings.launchAtLoginEnabled
        language = settings.language
        vocabularyText = settings.vocabularyHints.joined(separator: "\n")
        primaryProvider = settings.provider.primary
        fallbackProvider = settings.provider.fallback
        timeoutSeconds = settings.provider.timeoutSeconds
        groqModel = settings.provider.groqModel
        openAIModel = settings.provider.openAIModel

        hotkeysEnabled = !settings.hotkeys.isEmpty
        hotkeyPreset = detectPreset(hotkeys: settings.hotkeys)
        loadManualHotkeys(from: settings.hotkeys)
    }

    private struct EditorSnapshot: Equatable {
        let outputMode: OutputMode
        let recordingInteraction: RecordingInteractionMode
        let launchAtLoginEnabled: Bool
        let language: String
        let vocabularyHints: [String]
        let primaryProvider: ProviderKind
        let fallbackProvider: ProviderKind
        let timeoutSeconds: Int
        let groqModel: String
        let openAIModel: String
        let hotkeysEnabled: Bool
        let hotkeyPreset: HotkeyPreset
        let manualToggleHotkeyText: String
        let manualRetryHotkeyText: String
        let manualCancelHotkeyText: String
    }

    private func currentSnapshot() -> EditorSnapshot {
        EditorSnapshot(
            outputMode: outputMode,
            recordingInteraction: recordingInteraction,
            launchAtLoginEnabled: launchAtLoginEnabled,
            language: normalize(language),
            vocabularyHints: parseVocabularyHints(vocabularyText),
            primaryProvider: primaryProvider,
            fallbackProvider: fallbackProvider,
            timeoutSeconds: timeoutSeconds,
            groqModel: normalize(groqModel),
            openAIModel: normalize(openAIModel),
            hotkeysEnabled: hotkeysEnabled,
            hotkeyPreset: hotkeyPreset,
            manualToggleHotkeyText: normalizeHotkey(manualToggleHotkeyText),
            manualRetryHotkeyText: normalizeHotkey(manualRetryHotkeyText),
            manualCancelHotkeyText: normalizeHotkey(manualCancelHotkeyText)
        )
    }

    private func snapshot(from settings: AppSettings) -> EditorSnapshot {
        let byAction = Dictionary(uniqueKeysWithValues: settings.hotkeys.map { ($0.actionID, $0) })
        return EditorSnapshot(
            outputMode: settings.outputMode,
            recordingInteraction: settings.recordingInteraction,
            launchAtLoginEnabled: settings.launchAtLoginEnabled,
            language: normalize(settings.language),
            vocabularyHints: settings.vocabularyHints.map(normalize),
            primaryProvider: settings.provider.primary,
            fallbackProvider: settings.provider.fallback,
            timeoutSeconds: settings.provider.timeoutSeconds,
            groqModel: normalize(settings.provider.groqModel),
            openAIModel: normalize(settings.provider.openAIModel),
            hotkeysEnabled: !settings.hotkeys.isEmpty,
            hotkeyPreset: detectPreset(hotkeys: settings.hotkeys),
            manualToggleHotkeyText: normalizeHotkey(byAction["toggle"].flatMap(HotkeyCodec.render) ?? ""),
            manualRetryHotkeyText: normalizeHotkey(byAction["retry"].flatMap(HotkeyCodec.render) ?? ""),
            manualCancelHotkeyText: normalizeHotkey(byAction["cancel"].flatMap(HotkeyCodec.render) ?? "")
        )
    }

    private func normalize(_ value: String) -> String {
        value.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func normalizeHotkey(_ value: String) -> String {
        normalize(value).lowercased()
    }

    private func clipboardString() -> String? {
        NSPasteboard.general.string(forType: .string)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
