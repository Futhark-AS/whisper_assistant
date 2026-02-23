import XCTest
@testable import WhisperAssistantCore

final class HistoryStoreTests: XCTestCase {
    func testSaveAndListSession() async throws {
        let store = try HistoryStore()
        let sessionID = UUID()
        let tempAudio = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("history-test-\(sessionID.uuidString).caf")
        try Data("audio".utf8).write(to: tempAudio)

        let record = SessionRecord(
            sessionID: sessionID,
            createdAt: Date(),
            durationMS: 2000,
            providerPrimary: .groq,
            providerUsed: .groq,
            language: "auto",
            outputMode: .clipboard,
            status: .success,
            transcript: "hello world",
            audioPath: tempAudio
        )

        try await store.saveSession(record)
        let sessions = try await store.listSessions(limit: 20)
        XCTAssertTrue(sessions.contains(where: { $0.sessionID == sessionID }))
    }
}
