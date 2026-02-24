import XCTest
@testable import QuedoCore

final class PermissionRequirementsTests: XCTestCase {
    func testClipboardModeDoesNotRequireAccessibility() {
        let permissions = PermissionSnapshot(
            microphone: .granted,
            accessibility: .denied,
            inputMonitoring: .denied
        )

        XCTAssertTrue(
            permissions.satisfiesRuntimeRequirements(
                outputMode: .clipboard,
                buildProfile: .direct
            )
        )
    }

    func testClipboardAndPasteRequiresAccessibilityInDirectBuild() {
        let permissions = PermissionSnapshot(
            microphone: .granted,
            accessibility: .denied,
            inputMonitoring: .denied
        )

        XCTAssertFalse(
            permissions.satisfiesRuntimeRequirements(
                outputMode: .clipboardAndPaste,
                buildProfile: .direct
            )
        )
    }

    func testClipboardAndPasteDoesNotRequireAccessibilityInMASBuild() {
        let permissions = PermissionSnapshot(
            microphone: .granted,
            accessibility: .denied,
            inputMonitoring: .denied
        )

        XCTAssertTrue(
            permissions.satisfiesRuntimeRequirements(
                outputMode: .clipboardAndPaste,
                buildProfile: .mas
            )
        )
    }

    func testMicrophoneAlwaysRequired() {
        let permissions = PermissionSnapshot(
            microphone: .denied,
            accessibility: .granted,
            inputMonitoring: .granted
        )

        XCTAssertFalse(
            permissions.satisfiesRuntimeRequirements(
                outputMode: .clipboard,
                buildProfile: .direct
            )
        )
    }
}
