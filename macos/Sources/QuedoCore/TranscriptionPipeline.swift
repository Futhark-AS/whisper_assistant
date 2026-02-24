import Foundation

/// Errors returned by the transcription pipeline.
public enum TranscriptionPipelineError: Error, Sendable {
    /// No provider instance for requested kind.
    case providerUnavailable(ProviderKind)
    /// Both primary and fallback providers failed.
    case retryAvailable(primary: ProviderKind, fallback: ProviderKind)
    /// Audio file could not be chunked.
    case chunkingFailed
}

/// Finalized pipeline output.
public struct TranscriptionPipelineResult: Sendable {
    /// Final transcript text.
    public let text: String
    /// Provider used for final transcript.
    public let providerUsed: ProviderKind
    /// Indicates fallback path was used.
    public let fallbackUsed: Bool

    /// Creates a pipeline result.
    public init(text: String, providerUsed: ProviderKind, fallbackUsed: Bool) {
        self.text = text
        self.providerUsed = providerUsed
        self.fallbackUsed = fallbackUsed
    }
}

/// Orchestrates provider selection, retries, fallback, chunking, and cleanup.
public actor TranscriptionPipeline {
    private let providers: [ProviderKind: any TranscriptionProvider]
    private var fallbackStickyUntil: Date?
    private var primaryProbeTask: Task<Void, Never>?

    private let requestTimeoutSeconds: Int
    private let chunkByteLimit = 6_000_000

    /// Creates a transcription pipeline with registered providers.
    public init(providers: [any TranscriptionProvider], requestTimeoutSeconds: Int = 12) {
        var table: [ProviderKind: any TranscriptionProvider] = [:]
        for provider in providers {
            table[provider.kind] = provider
        }
        self.providers = table
        self.requestTimeoutSeconds = requestTimeoutSeconds
    }

    /// Performs transcription with primary retry and fallback policy.
    public func transcribe(
        audioFileURL: URL,
        settings: AppSettings,
        replacements: [String: String] = [:]
    ) async throws -> TranscriptionPipelineResult {
        let now = Date()
        let fallbackIsSticky = fallbackStickyUntil.map { now < $0 } ?? false

        let preferredPrimary = fallbackIsSticky ? settings.provider.fallback : settings.provider.primary
        let preferredFallback = fallbackIsSticky ? settings.provider.primary : settings.provider.fallback

        let chunks = try chunkAudioIfNeeded(audioFileURL)
        let primary = try provider(for: preferredPrimary)
        let fallback = try provider(for: preferredFallback)

        do {
            let text = try await runChunks(
                chunks: chunks,
                with: primary,
                model: model(for: preferredPrimary, settings: settings),
                language: settings.language,
                vocabularyHints: settings.vocabularyHints
            )
            let cleaned = cleanup(text: text, replacements: replacements)
            return TranscriptionPipelineResult(text: cleaned, providerUsed: preferredPrimary, fallbackUsed: false)
        } catch {
            let shouldRetryPrimary = isRetryable(error)
            if shouldRetryPrimary {
                try? await Task.sleep(for: .seconds(1))
                do {
                    let text = try await runChunks(
                        chunks: chunks,
                        with: primary,
                        model: model(for: preferredPrimary, settings: settings),
                        language: settings.language,
                        vocabularyHints: settings.vocabularyHints
                    )
                    let cleaned = cleanup(text: text, replacements: replacements)
                    return TranscriptionPipelineResult(text: cleaned, providerUsed: preferredPrimary, fallbackUsed: false)
                } catch {
                    do {
                        let text = try await runChunks(
                            chunks: chunks,
                            with: fallback,
                            model: model(for: preferredFallback, settings: settings),
                            language: settings.language,
                            vocabularyHints: settings.vocabularyHints
                        )
                        fallbackStickyUntil = Date().addingTimeInterval(30)
                        startPrimaryReprobe(provider: primary)
                        let cleaned = cleanup(text: text, replacements: replacements)
                        return TranscriptionPipelineResult(text: cleaned, providerUsed: preferredFallback, fallbackUsed: true)
                    } catch {
                        throw TranscriptionPipelineError.retryAvailable(primary: preferredPrimary, fallback: preferredFallback)
                    }
                }
            }

            do {
                let text = try await runChunks(
                    chunks: chunks,
                    with: fallback,
                    model: model(for: preferredFallback, settings: settings),
                    language: settings.language,
                    vocabularyHints: settings.vocabularyHints
                )
                fallbackStickyUntil = Date().addingTimeInterval(30)
                startPrimaryReprobe(provider: primary)
                let cleaned = cleanup(text: text, replacements: replacements)
                return TranscriptionPipelineResult(text: cleaned, providerUsed: preferredFallback, fallbackUsed: true)
            } catch {
                throw TranscriptionPipelineError.retryAvailable(primary: preferredPrimary, fallback: preferredFallback)
            }
        }
    }

    /// Performs a provider connectivity probe with 6-second timeout budget.
    public func connectivityCheck(primary: ProviderKind, fallback: ProviderKind) async -> (primaryOK: Bool, fallbackOK: Bool) {
        guard let primaryProvider = providers[primary], let fallbackProvider = providers[fallback] else {
            return (false, false)
        }

        async let primaryResult = primaryProvider.checkHealth(timeoutSeconds: 6)
        async let fallbackResult = fallbackProvider.checkHealth(timeoutSeconds: 6)
        return await (primaryResult, fallbackResult)
    }

    private func provider(for kind: ProviderKind) throws -> any TranscriptionProvider {
        guard let provider = providers[kind] else {
            throw TranscriptionPipelineError.providerUnavailable(kind)
        }
        return provider
    }

    private func runChunks(
        chunks: [URL],
        with provider: any TranscriptionProvider,
        model: String,
        language: String,
        vocabularyHints: [String]
    ) async throws -> String {
        var combined: [String] = []
        var rollingContext: String?

        for chunk in chunks {
            let request = TranscriptionRequest(
                audioFileURL: chunk,
                language: language,
                model: model,
                context: rollingContext,
                vocabularyHints: vocabularyHints
            )
            let response = try await transcribeChunkWithTimeout(request: request, provider: provider)
            combined.append(response.text)
            rollingContext = String(response.text.suffix(300))
        }

        return combined.joined(separator: " ")
    }

    /// Enforces a hard timeout around provider transcription to prevent indefinite hangs.
    private func transcribeChunkWithTimeout(
        request: TranscriptionRequest,
        provider: any TranscriptionProvider
    ) async throws -> TranscriptionResponse {
        let timeoutSeconds = requestTimeoutSeconds

        return try await withThrowingTaskGroup(of: TranscriptionResponse.self) { group in
            group.addTask {
                try await provider.transcribe(request: request)
            }
            group.addTask {
                try await Task.sleep(for: .seconds(Double(timeoutSeconds)))
                throw ProviderError.timeout
            }

            defer {
                group.cancelAll()
            }

            guard let response = try await group.next() else {
                throw ProviderError.networkFailure
            }
            return response
        }
    }

    private func chunkAudioIfNeeded(_ fileURL: URL) throws -> [URL] {
        let data = try Data(contentsOf: fileURL)
        if data.count <= chunkByteLimit {
            return [fileURL]
        }

        let tempRoot = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("QuedoChunks", isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)

        var files: [URL] = []
        var offset = 0
        var index = 0

        while offset < data.count {
            let end = min(offset + chunkByteLimit, data.count)
            let chunk = data.subdata(in: offset..<end)
            let path = tempRoot.appendingPathComponent("chunk-\(UUID().uuidString)-\(index).caf")
            do {
                try chunk.write(to: path, options: .atomic)
            } catch {
                throw TranscriptionPipelineError.chunkingFailed
            }
            files.append(path)
            offset = end
            index += 1
        }

        return files
    }

    private func cleanup(text: String, replacements: [String: String]) -> String {
        var output = text
            .replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "  ", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        let hallucinationPatterns = [
            "Thanks for watching.",
            "Thank you for watching.",
            "Subtitles by",
            "Please subscribe"
        ]

        for pattern in hallucinationPatterns {
            output = output.replacingOccurrences(of: pattern, with: "")
        }

        for (source, target) in replacements {
            output = output.replacingOccurrences(of: source, with: target)
        }

        return output
            .replacingOccurrences(of: "  ", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func model(for provider: ProviderKind, settings: AppSettings) -> String {
        switch provider {
        case .groq:
            return settings.provider.groqModel
        case .openAI:
            return settings.provider.openAIModel
        }
    }

    private func isRetryable(_ error: Error) -> Bool {
        guard let providerError = error as? ProviderError else {
            return false
        }

        switch providerError {
        case .timeout, .networkFailure, .transient:
            return true
        case .terminal, .missingAPIKey, .invalidResponse:
            return false
        }
    }

    private func startPrimaryReprobe(provider: any TranscriptionProvider) {
        primaryProbeTask?.cancel()
        primaryProbeTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(60))
                if await provider.checkHealth(timeoutSeconds: requestTimeoutSeconds) {
                    clearFallbackStickyWindow()
                    return
                }
            }
        }
    }

    private func clearFallbackStickyWindow() {
        fallbackStickyUntil = nil
    }
}
