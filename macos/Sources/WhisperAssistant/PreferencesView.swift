import SwiftUI
import WhisperAssistantCore

/// Preferences window for application configuration.
struct PreferencesView: View {
    @StateObject private var model: PreferencesViewModel

    init(configurationManager: ConfigurationManager) {
        _model = StateObject(wrappedValue: PreferencesViewModel(configurationManager: configurationManager))
    }

    var body: some View {
        Form {
            Picker("Interaction", selection: $model.recordingInteraction) {
                ForEach(RecordingInteractionMode.allCases, id: \.self) { mode in
                    Text(mode.rawValue.capitalized).tag(mode)
                }
            }

            Picker("Output", selection: $model.outputMode) {
                ForEach(OutputMode.allCases, id: \.self) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }

            TextField("Language", text: $model.language)

            TextField("Vocabulary (comma separated)", text: $model.vocabularyText)

            Picker("Primary Provider", selection: $model.primaryProvider) {
                ForEach(ProviderKind.allCases, id: \.self) { provider in
                    Text(provider.rawValue).tag(provider)
                }
            }

            Picker("Fallback Provider", selection: $model.fallbackProvider) {
                ForEach(ProviderKind.allCases, id: \.self) { provider in
                    Text(provider.rawValue).tag(provider)
                }
            }

            TextField("Groq Model", text: $model.groqModel)
            TextField("OpenAI Model", text: $model.openAIModel)

            HStack {
                Button("Save") {
                    Task {
                        await model.save()
                    }
                }
                .keyboardShortcut(.defaultAction)

                if let error = model.errorMessage {
                    Text(error)
                        .foregroundStyle(.red)
                }
            }
        }
        .padding(16)
        .frame(width: 480)
        .task {
            await model.load()
        }
    }
}

@MainActor
final class PreferencesViewModel: ObservableObject {
    @Published var outputMode: OutputMode = .clipboardAndPaste
    @Published var recordingInteraction: RecordingInteractionMode = .toggle
    @Published var language = "auto"
    @Published var vocabularyText = ""
    @Published var primaryProvider: ProviderKind = .groq
    @Published var fallbackProvider: ProviderKind = .openAI
    @Published var groqModel = "whisper-large-v3"
    @Published var openAIModel = "gpt-4o-mini-transcribe"
    @Published var errorMessage: String?

    private let configurationManager: ConfigurationManager
    private var loadedSettings = AppSettings.default

    init(configurationManager: ConfigurationManager) {
        self.configurationManager = configurationManager
    }

    func load() async {
        do {
            let settings = try await configurationManager.loadSettings()
            loadedSettings = settings
            outputMode = settings.outputMode
            recordingInteraction = settings.recordingInteraction
            language = settings.language
            vocabularyText = settings.vocabularyHints.joined(separator: ",")
            primaryProvider = settings.provider.primary
            fallbackProvider = settings.provider.fallback
            groqModel = settings.provider.groqModel
            openAIModel = settings.provider.openAIModel
            errorMessage = nil
        } catch {
            errorMessage = "Failed to load settings"
        }
    }

    func save() async {
        loadedSettings.outputMode = outputMode
        loadedSettings.recordingInteraction = recordingInteraction
        loadedSettings.language = language.trimmingCharacters(in: .whitespacesAndNewlines)
        loadedSettings.vocabularyHints = vocabularyText
            .split(separator: ",")
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        loadedSettings.provider.primary = primaryProvider
        loadedSettings.provider.fallback = fallbackProvider
        loadedSettings.provider.groqModel = groqModel.trimmingCharacters(in: .whitespacesAndNewlines)
        loadedSettings.provider.openAIModel = openAIModel.trimmingCharacters(in: .whitespacesAndNewlines)

        do {
            try await configurationManager.saveSettings(loadedSettings)
            errorMessage = nil
        } catch let error as SettingsValidationErrorSet {
            let summary = error.issues.map { "\($0.field): \($0.message)" }.joined(separator: " | ")
            errorMessage = summary
        } catch {
            errorMessage = "Failed to save settings"
        }
    }
}
