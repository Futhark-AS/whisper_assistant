import ArgumentParser
import Foundation
import WhisperAssistantCore

@main
struct WhisperAssistantCLI: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "wa",
        abstract: "Whisper Assistant companion CLI",
        subcommands: [
            Start.self,
            Stop.self,
            Status.self,
            Logs.self,
            Doctor.self,
            Config.self,
            History.self,
            Transcribe.self
        ]
    )
}

struct Start: AsyncParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Launch Whisper Assistant app")

    mutating func run() async throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        process.arguments = ["-a", "WhisperAssistant"]
        try process.run()
        process.waitUntilExit()
        print("start requested")
    }
}

struct Stop: AsyncParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Stop Whisper Assistant app")

    mutating func run() async throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        process.arguments = ["-x", "WhisperAssistant"]
        try process.run()
        process.waitUntilExit()
        print("stop requested")
    }
}

struct Status: AsyncParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Show runtime readiness status")

    mutating func run() async throws {
        let permissions = await PermissionCoordinator().checkAll()
        let settings = try await ConfigurationManager().redactedSnapshot()

        print("microphone=\(permissions.microphone.rawValue)")
        print("accessibility=\(permissions.accessibility.rawValue)")
        print("input_monitoring=\(permissions.inputMonitoring.rawValue)")
        print("settings=\(settings)")
    }
}

struct Logs: AsyncParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Show active logs")

    @Flag(help: "Show debug.log when available")
    var debug = false

    mutating func run() async throws {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Whisper Assistant/logs")
        let file = debug ? base.appendingPathComponent("debug.log") : base.appendingPathComponent("app.log")

        if !FileManager.default.fileExists(atPath: file.path) {
            print("log file not found: \(file.path)")
            return
        }

        let content = try String(contentsOf: file, encoding: .utf8)
        print(content)
    }
}

struct Doctor: AsyncParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Run diagnostics checks")

    mutating func run() async throws {
        let config = ConfigurationManager()
        let settings = try await config.loadSettings()

        let permissions = await PermissionCoordinator().checkAll()
        print("permissions.microphone=\(permissions.microphone.rawValue)")
        print("permissions.accessibility=\(permissions.accessibility.rawValue)")
        print("permissions.inputMonitoring=\(permissions.inputMonitoring.rawValue)")

        let groq = GroqProvider(timeoutSeconds: settings.provider.timeoutSeconds) {
            try await config.loadAPIKey(for: .groq) ?? ""
        }
        let openAI = OpenAIProvider(timeoutSeconds: settings.provider.timeoutSeconds) {
            try await config.loadAPIKey(for: .openAI) ?? ""
        }

        let pipeline = TranscriptionPipeline(providers: [groq, openAI], requestTimeoutSeconds: settings.provider.timeoutSeconds)
        let check = await pipeline.connectivityCheck(primary: settings.provider.primary, fallback: settings.provider.fallback)
        print("provider.primary.ok=\(check.primaryOK)")
        print("provider.fallback.ok=\(check.fallbackOK)")
    }
}

struct Config: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        abstract: "Configuration commands",
        subcommands: [Show.self]
    )

    struct Show: AsyncParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Print non-secret settings")

        mutating func run() async throws {
            let snapshot = try await ConfigurationManager().redactedSnapshot()
            for key in snapshot.keys.sorted() {
                if let value = snapshot[key] {
                    print("\(key)=\(value)")
                }
            }
        }
    }
}

struct History: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        abstract: "History commands",
        subcommands: [List.self, Play.self, TranscribeFromHistory.self]
    )

    struct List: AsyncParsableCommand {
        static let configuration = CommandConfiguration(abstract: "List recent sessions")

        @Option(help: "Maximum number of sessions")
        var limit: Int = 20

        mutating func run() async throws {
            let store = try HistoryStore()
            let sessions = try await store.listSessions(limit: limit)
            for (index, session) in sessions.enumerated() {
                print("\(index + 1). \(session.createdAt) \(session.providerUsed.rawValue) \(session.status.rawValue) \(session.transcriptPreview)")
            }
        }
    }

    struct Play: AsyncParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Play session audio by latest index")

        @Argument(help: "Session index from `history list` output")
        var index: Int

        mutating func run() async throws {
            let store = try HistoryStore()
            let sessions = try await store.listSessions(limit: max(index, 50))
            guard index > 0, index <= sessions.count else {
                throw ValidationError("Invalid history index")
            }

            let target = sessions[index - 1]
            let storageBase = await store.storageBasePath()
            let base = storageBase.appendingPathComponent("media/\(target.sessionID.uuidString)")
            let flac = base.appendingPathComponent("recording.flac")
            let wav = base.appendingPathComponent("recording.wav")
            let source: URL
            if FileManager.default.fileExists(atPath: flac.path) {
                source = flac
            } else if FileManager.default.fileExists(atPath: wav.path) {
                source = wav
            } else {
                throw ValidationError("No media file found for selected session")
            }

            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/afplay")
            process.arguments = [source.path]
            try process.run()
            process.waitUntilExit()
        }
    }

    struct TranscribeFromHistory: AsyncParsableCommand {
        static let configuration = CommandConfiguration(commandName: "transcribe", abstract: "Re-transcribe a history item")

        @Argument(help: "Session index from `history list` output")
        var index: Int

        @Option(help: "Language override")
        var language: String?

        mutating func run() async throws {
            let store = try HistoryStore()
            let sessions = try await store.listSessions(limit: max(index, 50))
            guard index > 0, index <= sessions.count else {
                throw ValidationError("Invalid history index")
            }

            let target = sessions[index - 1]
            let storageBase = await store.storageBasePath()
            let base = storageBase.appendingPathComponent("media/\(target.sessionID.uuidString)")
            let flac = base.appendingPathComponent("recording.flac")
            let wav = base.appendingPathComponent("recording.wav")
            let source: URL
            if FileManager.default.fileExists(atPath: flac.path) {
                source = flac
            } else if FileManager.default.fileExists(atPath: wav.path) {
                source = wav
            } else {
                throw ValidationError("No media file found for selected session")
            }

            var command = Transcribe()
            command.file = source.path
            command.language = language
            try await command.run()
        }
    }
}

struct Transcribe: AsyncParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Transcribe an audio file")

    @Argument(help: "Path to local audio file")
    var file: String

    @Option(help: "Language override")
    var language: String?

    mutating func run() async throws {
        let source = URL(fileURLWithPath: file)
        guard FileManager.default.fileExists(atPath: source.path) else {
            throw ValidationError("File does not exist: \(source.path)")
        }

        let config = ConfigurationManager()
        var settings = try await config.loadSettings()
        if let language, !language.isEmpty {
            settings.language = language
        }

        let groq = GroqProvider(timeoutSeconds: settings.provider.timeoutSeconds) {
            try await config.loadAPIKey(for: .groq) ?? ""
        }
        let openAI = OpenAIProvider(timeoutSeconds: settings.provider.timeoutSeconds) {
            try await config.loadAPIKey(for: .openAI) ?? ""
        }

        let pipeline = TranscriptionPipeline(providers: [groq, openAI], requestTimeoutSeconds: settings.provider.timeoutSeconds)
        let result = try await pipeline.transcribe(audioFileURL: source, settings: settings)
        print(result.text)
    }
}
