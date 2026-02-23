import XCTest
@testable import WhisperAssistantCore

final class StateMachineTests: XCTestCase {
    func testBootingToReadyTransition() async throws {
        let machine = LifecycleStateMachine()
        try await machine.transition(to: .ready)
        let snapshot = await machine.snapshot()
        XCTAssertEqual(snapshot.phase, .ready)
    }

    func testReadyToArmingRequiresActiveSession() async throws {
        let machine = LifecycleStateMachine()
        try await machine.transition(to: .ready)

        do {
            try await machine.transition(to: .arming)
            XCTFail("Transition should fail without active session")
        } catch {
            XCTAssertTrue(error is StateTransitionError)
        }

        let sessionID = UUID()
        try await machine.beginSession(id: sessionID)
        try await machine.transition(to: .arming)
        let snapshot = await machine.snapshot()
        XCTAssertEqual(snapshot.phase, .arming)
        XCTAssertEqual(snapshot.currentSessionID, sessionID)
    }

    func testRetryAvailableToProcessingTransition() async throws {
        let machine = LifecycleStateMachine()
        try await machine.transition(to: .ready)
        try await machine.transition(to: .degraded, degradedReason: .providerUnavailable)
        try await machine.transition(to: .ready)

        try await machine.beginSession(id: UUID())
        try await machine.transition(to: .arming)
        try await machine.transition(to: .recording)
        try await machine.transition(to: .processing)
        try await machine.transition(to: .providerFallback)
        try await machine.transition(to: .retryAvailable)

        try await machine.transition(to: .processing)
        let snapshot = await machine.snapshot()
        XCTAssertEqual(snapshot.phase, .processing)
    }

    func testUIContractForPermissionsDegradedState() async {
        let contract = LifecycleStateMachine.uiContract(for: .degraded, degradedReason: .permissions)
        XCTAssertEqual(contract.icon, "shield")
        XCTAssertEqual(contract.notificationCopy, "Permission needed for full functionality.")
        XCTAssertTrue(contract.actions.contains(.openSettings))
    }
}
