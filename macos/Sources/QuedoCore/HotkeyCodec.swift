import Carbon
import Foundation

/// Shared parser/renderer for human-editable hotkey strings.
public enum HotkeyCodec {
    private static let modifierByToken: [String: HotkeyModifiers] = [
        "cmd": .command,
        "command": .command,
        "alt": .option,
        "option": .option,
        "ctrl": .control,
        "control": .control,
        "shift": .shift,
        "fn": .function,
        "function": .function
    ]

    private static let tokenToKeyCode: [String: UInt32] = [
        // letters
        "a": UInt32(kVK_ANSI_A),
        "b": UInt32(kVK_ANSI_B),
        "c": UInt32(kVK_ANSI_C),
        "d": UInt32(kVK_ANSI_D),
        "e": UInt32(kVK_ANSI_E),
        "f": UInt32(kVK_ANSI_F),
        "g": UInt32(kVK_ANSI_G),
        "h": UInt32(kVK_ANSI_H),
        "i": UInt32(kVK_ANSI_I),
        "j": UInt32(kVK_ANSI_J),
        "k": UInt32(kVK_ANSI_K),
        "l": UInt32(kVK_ANSI_L),
        "m": UInt32(kVK_ANSI_M),
        "n": UInt32(kVK_ANSI_N),
        "o": UInt32(kVK_ANSI_O),
        "p": UInt32(kVK_ANSI_P),
        "q": UInt32(kVK_ANSI_Q),
        "r": UInt32(kVK_ANSI_R),
        "s": UInt32(kVK_ANSI_S),
        "t": UInt32(kVK_ANSI_T),
        "u": UInt32(kVK_ANSI_U),
        "v": UInt32(kVK_ANSI_V),
        "w": UInt32(kVK_ANSI_W),
        "x": UInt32(kVK_ANSI_X),
        "y": UInt32(kVK_ANSI_Y),
        "z": UInt32(kVK_ANSI_Z),

        // digits
        "0": UInt32(kVK_ANSI_0),
        "1": UInt32(kVK_ANSI_1),
        "2": UInt32(kVK_ANSI_2),
        "3": UInt32(kVK_ANSI_3),
        "4": UInt32(kVK_ANSI_4),
        "5": UInt32(kVK_ANSI_5),
        "6": UInt32(kVK_ANSI_6),
        "7": UInt32(kVK_ANSI_7),
        "8": UInt32(kVK_ANSI_8),
        "9": UInt32(kVK_ANSI_9),

        // punctuation
        "-": UInt32(kVK_ANSI_Minus),
        "=": UInt32(kVK_ANSI_Equal),
        "[": UInt32(kVK_ANSI_LeftBracket),
        "]": UInt32(kVK_ANSI_RightBracket),
        "\\": UInt32(kVK_ANSI_Backslash),
        ";": UInt32(kVK_ANSI_Semicolon),
        "'": UInt32(kVK_ANSI_Quote),
        ",": UInt32(kVK_ANSI_Comma),
        ".": UInt32(kVK_ANSI_Period),
        "/": UInt32(kVK_ANSI_Slash),
        "`": UInt32(kVK_ANSI_Grave),

        // common named keys
        "space": UInt32(kVK_Space),
        "tab": UInt32(kVK_Tab),
        "return": UInt32(kVK_Return),
        "enter": UInt32(kVK_Return),
        "escape": UInt32(kVK_Escape),
        "esc": UInt32(kVK_Escape),
        "delete": UInt32(kVK_Delete),
        "backspace": UInt32(kVK_Delete),
        "forwarddelete": UInt32(kVK_ForwardDelete),
        "help": UInt32(kVK_Help),
        "home": UInt32(kVK_Home),
        "end": UInt32(kVK_End),
        "pageup": UInt32(kVK_PageUp),
        "pagedown": UInt32(kVK_PageDown),

        // arrows
        "left": UInt32(kVK_LeftArrow),
        "right": UInt32(kVK_RightArrow),
        "up": UInt32(kVK_UpArrow),
        "down": UInt32(kVK_DownArrow),

        // function row
        "f1": UInt32(kVK_F1),
        "f2": UInt32(kVK_F2),
        "f3": UInt32(kVK_F3),
        "f4": UInt32(kVK_F4),
        "f5": UInt32(kVK_F5),
        "f6": UInt32(kVK_F6),
        "f7": UInt32(kVK_F7),
        "f8": UInt32(kVK_F8),
        "f9": UInt32(kVK_F9),
        "f10": UInt32(kVK_F10),
        "f11": UInt32(kVK_F11),
        "f12": UInt32(kVK_F12),
        "f13": UInt32(kVK_F13),
        "f14": UInt32(kVK_F14),
        "f15": UInt32(kVK_F15),
        "f16": UInt32(kVK_F16),
        "f17": UInt32(kVK_F17),
        "f18": UInt32(kVK_F18),
        "f19": UInt32(kVK_F19),
        "f20": UInt32(kVK_F20)
    ]

    private static let keyCodeToToken: [UInt32: String] = [
        UInt32(kVK_ANSI_A): "a",
        UInt32(kVK_ANSI_B): "b",
        UInt32(kVK_ANSI_C): "c",
        UInt32(kVK_ANSI_D): "d",
        UInt32(kVK_ANSI_E): "e",
        UInt32(kVK_ANSI_F): "f",
        UInt32(kVK_ANSI_G): "g",
        UInt32(kVK_ANSI_H): "h",
        UInt32(kVK_ANSI_I): "i",
        UInt32(kVK_ANSI_J): "j",
        UInt32(kVK_ANSI_K): "k",
        UInt32(kVK_ANSI_L): "l",
        UInt32(kVK_ANSI_M): "m",
        UInt32(kVK_ANSI_N): "n",
        UInt32(kVK_ANSI_O): "o",
        UInt32(kVK_ANSI_P): "p",
        UInt32(kVK_ANSI_Q): "q",
        UInt32(kVK_ANSI_R): "r",
        UInt32(kVK_ANSI_S): "s",
        UInt32(kVK_ANSI_T): "t",
        UInt32(kVK_ANSI_U): "u",
        UInt32(kVK_ANSI_V): "v",
        UInt32(kVK_ANSI_W): "w",
        UInt32(kVK_ANSI_X): "x",
        UInt32(kVK_ANSI_Y): "y",
        UInt32(kVK_ANSI_Z): "z",
        UInt32(kVK_ANSI_0): "0",
        UInt32(kVK_ANSI_1): "1",
        UInt32(kVK_ANSI_2): "2",
        UInt32(kVK_ANSI_3): "3",
        UInt32(kVK_ANSI_4): "4",
        UInt32(kVK_ANSI_5): "5",
        UInt32(kVK_ANSI_6): "6",
        UInt32(kVK_ANSI_7): "7",
        UInt32(kVK_ANSI_8): "8",
        UInt32(kVK_ANSI_9): "9",
        UInt32(kVK_ANSI_Minus): "-",
        UInt32(kVK_ANSI_Equal): "=",
        UInt32(kVK_ANSI_LeftBracket): "[",
        UInt32(kVK_ANSI_RightBracket): "]",
        UInt32(kVK_ANSI_Backslash): "\\",
        UInt32(kVK_ANSI_Semicolon): ";",
        UInt32(kVK_ANSI_Quote): "'",
        UInt32(kVK_ANSI_Comma): ",",
        UInt32(kVK_ANSI_Period): ".",
        UInt32(kVK_ANSI_Slash): "/",
        UInt32(kVK_ANSI_Grave): "`",
        UInt32(kVK_Space): "space",
        UInt32(kVK_Tab): "tab",
        UInt32(kVK_Return): "return",
        UInt32(kVK_Escape): "escape",
        UInt32(kVK_Delete): "delete",
        UInt32(kVK_ForwardDelete): "forwarddelete",
        UInt32(kVK_Help): "help",
        UInt32(kVK_Home): "home",
        UInt32(kVK_End): "end",
        UInt32(kVK_PageUp): "pageup",
        UInt32(kVK_PageDown): "pagedown",
        UInt32(kVK_LeftArrow): "left",
        UInt32(kVK_RightArrow): "right",
        UInt32(kVK_UpArrow): "up",
        UInt32(kVK_DownArrow): "down",
        UInt32(kVK_F1): "f1",
        UInt32(kVK_F2): "f2",
        UInt32(kVK_F3): "f3",
        UInt32(kVK_F4): "f4",
        UInt32(kVK_F5): "f5",
        UInt32(kVK_F6): "f6",
        UInt32(kVK_F7): "f7",
        UInt32(kVK_F8): "f8",
        UInt32(kVK_F9): "f9",
        UInt32(kVK_F10): "f10",
        UInt32(kVK_F11): "f11",
        UInt32(kVK_F12): "f12",
        UInt32(kVK_F13): "f13",
        UInt32(kVK_F14): "f14",
        UInt32(kVK_F15): "f15",
        UInt32(kVK_F16): "f16",
        UInt32(kVK_F17): "f17",
        UInt32(kVK_F18): "f18",
        UInt32(kVK_F19): "f19",
        UInt32(kVK_F20): "f20"
    ]

    private static let displayByToken: [String: String] = [
        "space": "Space",
        "tab": "Tab",
        "return": "Return",
        "escape": "Esc",
        "delete": "Delete",
        "forwarddelete": "Forward Delete",
        "left": "Left",
        "right": "Right",
        "up": "Up",
        "down": "Down",
        "pageup": "Page Up",
        "pagedown": "Page Down"
    ]

    /// Parses a string like `cmd+shift+r` or `fn+ctrl` into a hotkey binding.
    public static func parse(_ value: String, actionID: String) -> HotkeyBinding? {
        let tokens = value
            .lowercased()
            .split(separator: "+")
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        guard !tokens.isEmpty else {
            return nil
        }

        var modifiers: HotkeyModifiers = []
        var keyCode: UInt32?

        for token in tokens {
            if let modifier = modifierByToken[token] {
                modifiers.insert(modifier)
                continue
            }

            guard let parsedKeyCode = keyCodeForToken(token) else {
                return nil
            }

            if let existing = keyCode, existing != parsedKeyCode {
                return nil
            }
            keyCode = parsedKeyCode
        }

        if keyCode == nil {
            guard !modifiers.isEmpty else {
                return nil
            }
            keyCode = HotkeyBinding.modifiersOnlyKeyCode
        }

        return HotkeyBinding(actionID: actionID, keyCode: keyCode!, modifiers: modifiers)
    }

    /// Renders a hotkey binding back to env-compatible text.
    public static func render(_ binding: HotkeyBinding) -> String? {
        var tokens: [String] = []
        if binding.modifiers.contains(.function) { tokens.append("fn") }
        if binding.modifiers.contains(.control) { tokens.append("ctrl") }
        if binding.modifiers.contains(.shift) { tokens.append("shift") }
        if binding.modifiers.contains(.option) { tokens.append("alt") }
        if binding.modifiers.contains(.command) { tokens.append("cmd") }

        if binding.keyCode == HotkeyBinding.modifiersOnlyKeyCode {
            return tokens.isEmpty ? nil : tokens.joined(separator: "+")
        }

        let keyToken = keyCodeToToken[binding.keyCode] ?? "keycode:\(binding.keyCode)"
        tokens.append(keyToken)
        return tokens.joined(separator: "+")
    }

    /// Human readable display text, for example `Fn+Ctrl+R`.
    public static func displayString(_ binding: HotkeyBinding) -> String {
        var components: [String] = []
        if binding.modifiers.contains(.function) { components.append("Fn") }
        if binding.modifiers.contains(.control) { components.append("Ctrl") }
        if binding.modifiers.contains(.option) { components.append("Opt") }
        if binding.modifiers.contains(.shift) { components.append("Shift") }
        if binding.modifiers.contains(.command) { components.append("Cmd") }

        if binding.keyCode != HotkeyBinding.modifiersOnlyKeyCode {
            if let token = keyCodeToToken[binding.keyCode] {
                components.append(displayByToken[token] ?? token.uppercased())
            } else {
                components.append("KeyCode\(binding.keyCode)")
            }
        }

        return components.joined(separator: "+")
    }

    private static func keyCodeForToken(_ token: String) -> UInt32? {
        if let value = tokenToKeyCode[token] {
            return value
        }

        if token.hasPrefix("keycode:") {
            let raw = token.dropFirst("keycode:".count)
            return UInt32(raw)
        }

        if token.hasPrefix("vk:") {
            let raw = token.dropFirst("vk:".count)
            return UInt32(raw)
        }

        return nil
    }
}
