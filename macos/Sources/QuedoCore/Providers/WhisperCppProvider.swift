import Foundation

/// Local whisper.cpp transcription provider implementation.
public struct WhisperCppProvider: TranscriptionProvider {
    /// Provider family identifier.
    public let kind: ProviderKind = .whisperCpp

    private let timeoutSeconds: Int
    private let executablePath: String
    private let modelPathProvider: @Sendable () async throws -> String

    /// Creates a whisper.cpp provider.
    public init(
        timeoutSeconds: Int,
        executablePath: String = "whisper-cli",
        modelPathProvider: @escaping @Sendable () async throws -> String = {
            ProviderConfiguration.defaultValue.whisperCppModelPath
        }
    ) {
        self.timeoutSeconds = timeoutSeconds
        self.executablePath = executablePath
        self.modelPathProvider = modelPathProvider
    }

    /// Transcribes audio with the local whisper.cpp CLI.
    public func transcribe(request: TranscriptionRequest) async throws -> TranscriptionResponse {
        guard let executableURL = resolveExecutableURL() else {
            throw ProviderError.terminal(statusCode: 127, message: "whisper-cli not found in PATH")
        }

        guard let modelURL = resolveModelURL(from: request.model) else {
            throw ProviderError.terminal(
                statusCode: 2,
                message: "whisper.cpp model not found: \(request.model)"
            )
        }

        let fileManager = FileManager.default
        let outputDirectory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("QuedoWhisperCpp", isDirectory: true)
        try fileManager.createDirectory(at: outputDirectory, withIntermediateDirectories: true)
        let outputPrefix = outputDirectory.appendingPathComponent(UUID().uuidString)
        let outputTextFile = outputPrefix.appendingPathExtension("txt")
        defer {
            try? fileManager.removeItem(at: outputTextFile)
        }

        var arguments: [String] = [
            "-m", modelURL.path,
            "-f", request.audioFileURL.path,
            "-l", request.language,
            "-otxt",
            "-nt",
            "-np",
            "-of", outputPrefix.path
        ]

        let context = request.context?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let prompt = buildPrompt(context: context, vocabularyHints: request.vocabularyHints)
        if !prompt.isEmpty {
            arguments.append(contentsOf: ["--prompt", prompt])
        }

        let process = Process()
        process.executableURL = executableURL
        process.arguments = arguments
        process.currentDirectoryURL = outputDirectory

        var environment = ProcessInfo.processInfo.environment
        environment["NO_COLOR"] = "1"
        process.environment = environment

        let stderrPipe = Pipe()
        process.standardOutput = Pipe()
        process.standardError = stderrPipe

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            throw ProviderError.networkFailure
        }

        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
        let stderr = String(data: stderrData, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        if process.terminationStatus != 0 {
            throw ProviderError.terminal(
                statusCode: Int(process.terminationStatus),
                message: stderr.isEmpty ? "whisper-cli failed" : stderr
            )
        }

        guard fileManager.fileExists(atPath: outputTextFile.path) else {
            throw ProviderError.invalidResponse
        }

        let rawText = try String(contentsOf: outputTextFile, encoding: .utf8)
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        return TranscriptionResponse(text: text, provider: .whisperCpp, isPartial: false)
    }

    /// Checks if whisper.cpp executable and configured model are available.
    public func checkHealth(timeoutSeconds: Int) async -> Bool {
        _ = timeoutSeconds
        guard resolveExecutableURL() != nil else {
            return false
        }
        let modelPath: String
        do {
            modelPath = try await modelPathProvider()
        } catch {
            return false
        }
        return resolveModelURL(from: modelPath) != nil
    }

    private func buildPrompt(context: String, vocabularyHints: [String]) -> String {
        var parts: [String] = []
        if !context.isEmpty {
            parts.append(context)
        }
        if !vocabularyHints.isEmpty {
            parts.append(vocabularyHints.joined(separator: ", "))
        }
        return parts.joined(separator: "\n")
    }

    private func resolveExecutableURL() -> URL? {
        let fileManager = FileManager.default
        let expanded = (executablePath as NSString).expandingTildeInPath
        if expanded.contains("/") {
            return fileManager.isExecutableFile(atPath: expanded) ? URL(fileURLWithPath: expanded) : nil
        }

        let pathValue = ProcessInfo.processInfo.environment["PATH"] ?? ""
        let pathSegments = pathValue
            .split(separator: ":")
            .map(String.init)
            .filter { !$0.isEmpty }
        for segment in pathSegments {
            let candidate = URL(fileURLWithPath: segment, isDirectory: true)
                .appendingPathComponent(expanded)
            if fileManager.isExecutableFile(atPath: candidate.path) {
                return candidate
            }
        }

        let fallbackPaths = [
            "/opt/homebrew/bin/\(expanded)",
            "/usr/local/bin/\(expanded)"
        ]
        for fallback in fallbackPaths where fileManager.isExecutableFile(atPath: fallback) {
            return URL(fileURLWithPath: fallback)
        }
        return nil
    }

    private func resolveModelURL(from rawModel: String) -> URL? {
        let fileManager = FileManager.default
        let trimmed = rawModel.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }

        let expanded = (trimmed as NSString).expandingTildeInPath
        let directPath: String
        if expanded.hasPrefix("/") {
            directPath = expanded
        } else {
            directPath = URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true)
                .appendingPathComponent(expanded)
                .path
        }

        if fileManager.fileExists(atPath: directPath) {
            return URL(fileURLWithPath: directPath)
        }

        guard !expanded.contains("/") else {
            return nil
        }

        let candidateDirectories: [URL] = [
            URL(fileURLWithPath: "/opt/homebrew/opt/whisper-cpp/share/whisper/models", isDirectory: true),
            URL(fileURLWithPath: "/usr/local/opt/whisper-cpp/share/whisper/models", isDirectory: true),
            URL(fileURLWithPath: "/opt/homebrew/share/whisper/models", isDirectory: true),
            URL(fileURLWithPath: "/usr/local/share/whisper/models", isDirectory: true),
            FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".cache/whisper", isDirectory: true)
        ]

        for directory in candidateDirectories {
            let candidate = directory.appendingPathComponent(expanded)
            if fileManager.fileExists(atPath: candidate.path) {
                return candidate
            }
        }
        return nil
    }
}
