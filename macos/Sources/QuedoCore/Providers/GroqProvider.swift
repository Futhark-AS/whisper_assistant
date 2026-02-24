import Foundation

/// Groq transcription provider implementation.
public struct GroqProvider: TranscriptionProvider {
    /// Provider family identifier.
    public let kind: ProviderKind = .groq

    private let session: URLSession
    private let timeoutSeconds: Int
    private let apiKeyProvider: @Sendable () async throws -> String

    /// Creates a Groq provider.
    public init(
        session: URLSession = .shared,
        timeoutSeconds: Int,
        apiKeyProvider: @escaping @Sendable () async throws -> String
    ) {
        self.session = session
        self.timeoutSeconds = timeoutSeconds
        self.apiKeyProvider = apiKeyProvider
    }

    /// Transcribes audio with the Groq API.
    public func transcribe(request: TranscriptionRequest) async throws -> TranscriptionResponse {
        let apiKey = try await apiKeyProvider()
        guard !apiKey.isEmpty else {
            throw ProviderError.missingAPIKey
        }

        var multipart = MultipartFormData()
        multipart.addField(name: "model", value: request.model)
        if request.language != "auto" {
            multipart.addField(name: "language", value: request.language)
        }
        if let context = request.context, !context.isEmpty {
            multipart.addField(name: "prompt", value: context)
        }

        let audioData = try Data(contentsOf: request.audioFileURL)
        multipart.addFile(name: "file", filename: request.audioFileURL.lastPathComponent, mimeType: "audio/x-caf", data: audioData)
        multipart.finalize()

        guard let endpoint = URL(string: "https://api.groq.com/openai/v1/audio/transcriptions") else {
            throw ProviderError.invalidResponse
        }

        var requestObject = URLRequest(url: endpoint)
        requestObject.httpMethod = "POST"
        requestObject.timeoutInterval = TimeInterval(timeoutSeconds)
        requestObject.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        requestObject.setValue("multipart/form-data; boundary=\(multipart.boundary)", forHTTPHeaderField: "Content-Type")
        requestObject.httpBody = multipart.data()

        do {
            let (data, response) = try await session.data(for: requestObject)
            guard let http = response as? HTTPURLResponse else {
                throw ProviderError.invalidResponse
            }

            switch http.statusCode {
            case 200:
                let decoded = try JSONDecoder().decode(ProviderTextResponse.self, from: data)
                return TranscriptionResponse(text: decoded.text, provider: .groq, isPartial: false)
            case 408, 429, 500, 502, 503, 504:
                throw ProviderError.transient(statusCode: http.statusCode)
            default:
                let body = String(data: data, encoding: .utf8) ?? "unknown"
                throw ProviderError.terminal(statusCode: http.statusCode, message: body)
            }
        } catch let urlError as URLError where urlError.code == .timedOut {
            throw ProviderError.timeout
        } catch let provider as ProviderError {
            throw provider
        } catch {
            throw ProviderError.networkFailure
        }
    }

    /// Checks if Groq endpoint is reachable.
    public func checkHealth(timeoutSeconds: Int) async -> Bool {
        let apiKey: String
        do {
            apiKey = try await apiKeyProvider()
        } catch {
            return false
        }

        guard !apiKey.isEmpty else {
            return false
        }

        guard let url = URL(string: "https://api.groq.com/openai/v1/models") else {
            return false
        }

        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.timeoutInterval = TimeInterval(timeoutSeconds)
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")

        do {
            let (_, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else {
                return false
            }
            return (200..<300).contains(http.statusCode)
        } catch {
            return false
        }
    }
}

private struct ProviderTextResponse: Codable {
    let text: String
}
