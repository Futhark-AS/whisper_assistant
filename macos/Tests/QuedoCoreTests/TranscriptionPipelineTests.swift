import XCTest
@testable import QuedoCore

final class TranscriptionPipelineTests: XCTestCase {
    func testFallbackAfterPrimaryFailure() async throws {
        let file = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("pipeline-test-\(UUID().uuidString).caf")
        try Data("audio".utf8).write(to: file)

        let primary = MockProvider(kind: .groq, mode: .alwaysFail)
        let fallback = MockProvider(kind: .openAI, mode: .alwaysSucceed("fallback transcript"))
        let pipeline = TranscriptionPipeline(providers: [primary, fallback])

        let result = try await pipeline.transcribe(audioFileURL: file, settings: .default)
        XCTAssertEqual(result.providerUsed, .openAI)
        XCTAssertTrue(result.fallbackUsed)
        XCTAssertEqual(result.text, "fallback transcript")
    }

    func testCleanupRemovesKnownHallucinations() async throws {
        let file = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("pipeline-test-cleanup-\(UUID().uuidString).caf")
        try Data("audio".utf8).write(to: file)

        let provider = MockProvider(kind: .groq, mode: .alwaysSucceed("Thanks for watching. hello world"))
        let backup = MockProvider(kind: .openAI, mode: .alwaysSucceed("unused"))
        let pipeline = TranscriptionPipeline(providers: [provider, backup])

        var settings = AppSettings.default
        settings.provider.primary = .groq
        settings.provider.fallback = .openAI

        let result = try await pipeline.transcribe(audioFileURL: file, settings: settings)
        XCTAssertEqual(result.text, "hello world")
    }
}

private struct MockProvider: TranscriptionProvider {
    enum Mode {
        case alwaysFail
        case alwaysSucceed(String)
    }

    let kind: ProviderKind
    let mode: Mode

    func transcribe(request: TranscriptionRequest) async throws -> TranscriptionResponse {
        _ = request
        switch mode {
        case .alwaysFail:
            throw ProviderError.transient(statusCode: 503)
        case let .alwaysSucceed(text):
            return TranscriptionResponse(text: text, provider: kind, isPartial: false)
        }
    }

    func checkHealth(timeoutSeconds: Int) async -> Bool {
        _ = timeoutSeconds
        switch mode {
        case .alwaysFail:
            return false
        case .alwaysSucceed:
            return true
        }
    }
}
