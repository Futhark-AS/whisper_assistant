import XCTest
@testable import WhisperAssistantCore

final class HotkeyCodecTests: XCTestCase {
    func testParseStandardChord() {
        let binding = HotkeyCodec.parse("cmd+shift+r", actionID: "toggle")
        XCTAssertEqual(binding?.actionID, "toggle")
        XCTAssertEqual(binding?.keyCode, 15)
        XCTAssertEqual(binding?.modifiers, [.command, .shift])
    }

    func testParseModifierOnlyChord() {
        let binding = HotkeyCodec.parse("fn+ctrl", actionID: "toggle")
        XCTAssertEqual(binding?.keyCode, HotkeyBinding.modifiersOnlyKeyCode)
        XCTAssertEqual(binding?.modifiers, [.function, .control])
    }

    func testParseWithKeycodeToken() {
        let binding = HotkeyCodec.parse("ctrl+keycode:123", actionID: "retry")
        XCTAssertEqual(binding?.keyCode, 123)
        XCTAssertEqual(binding?.modifiers, [.control])
    }

    func testRenderRoundTrip() {
        let original = HotkeyBinding(actionID: "cancel", keyCode: 126, modifiers: [.control, .option])
        let rendered = HotkeyCodec.render(original)
        XCTAssertEqual(rendered, "ctrl+alt+up")

        let parsed = rendered.flatMap { HotkeyCodec.parse($0, actionID: "cancel") }
        XCTAssertEqual(parsed, original)
    }
}
