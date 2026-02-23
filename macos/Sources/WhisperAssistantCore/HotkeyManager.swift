import AppKit
import Carbon
import Foundation

/// Errors thrown by global hotkey registration.
public enum HotkeyError: Error, Sendable {
    /// Requested key combination is already owned by another app.
    case conflict
    /// Registration failed for an unknown reason.
    case registrationFailed(status: OSStatus)
    /// Event handler could not be installed.
    case eventHandlerInstallFailed(status: OSStatus)
}

/// Carbon-based hotkey manager for direct and MAS builds.
public final class HotkeyManager: @unchecked Sendable {
    private static let signature: OSType = 0x57415354 // WAST

    private var handlerRef: EventHandlerRef?
    private var hotkeyRefs: [UInt32: EventHotKeyRef] = [:]
    private var actionByID: [UInt32: String] = [:]
    private var registeredBindings: [HotkeyBinding] = []
    private var onHotkey: (@Sendable (String) -> Void)?
    private let lock = NSLock()
    private var wakeObserver: NSObjectProtocol?

    /// Creates a hotkey manager and installs event callback handler.
    public init() throws {
        try installEventHandler()
        installWakeObserver()
    }

    deinit {
        if let wakeObserver {
            NotificationCenter.default.removeObserver(wakeObserver)
        }
        unregisterAll()
        if let handlerRef {
            RemoveEventHandler(handlerRef)
        }
    }

    /// Activates hotkey bindings with bounded retry.
    public func setBindings(_ bindings: [HotkeyBinding], handler: @escaping @Sendable (String) -> Void) async throws {
        lock.lock()
        self.registeredBindings = bindings
        self.onHotkey = handler
        lock.unlock()

        do {
            try registerAll(bindings: bindings)
        } catch {
            try? await Task.sleep(for: .milliseconds(300))
            try registerAll(bindings: bindings)
        }
    }

    /// Unregisters all active hotkeys.
    public func deactivate() {
        unregisterAll()
    }

    /// Retries registration after wake with bounded delays.
    public func recoverAfterWake() async {
        let bindings: [HotkeyBinding]
        lock.lock()
        bindings = registeredBindings
        lock.unlock()

        guard !bindings.isEmpty else {
            return
        }

        for delay in [300, 1000] {
            do {
                try registerAll(bindings: bindings)
                return
            } catch {
                try? await Task.sleep(for: .milliseconds(delay))
            }
        }
    }

    private func installEventHandler() throws {
        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let status = InstallEventHandler(
            GetApplicationEventTarget(),
            { _, event, userData in
                guard let userData, let event else {
                    return noErr
                }
                let manager = Unmanaged<HotkeyManager>.fromOpaque(userData).takeUnretainedValue()
                manager.handleHotkeyEvent(event)
                return noErr
            },
            1,
            &eventType,
            UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque()),
            &handlerRef
        )

        guard status == noErr else {
            throw HotkeyError.eventHandlerInstallFailed(status: status)
        }
    }

    private func registerAll(bindings: [HotkeyBinding]) throws {
        unregisterAll()

        for (index, binding) in bindings.enumerated() {
            var ref: EventHotKeyRef?
            var identifier = EventHotKeyID(signature: Self.signature, id: UInt32(index + 1))
            let status = RegisterEventHotKey(
                UInt32(binding.keyCode),
                carbonModifiers(from: binding.modifiers),
                identifier,
                GetApplicationEventTarget(),
                0,
                &ref
            )

            if status == eventHotKeyExistsErr {
                unregisterAll()
                throw HotkeyError.conflict
            }

            guard status == noErr, let ref else {
                unregisterAll()
                throw HotkeyError.registrationFailed(status: status)
            }

            hotkeyRefs[identifier.id] = ref
            actionByID[identifier.id] = binding.actionID
        }
    }

    private func unregisterAll() {
        for (_, ref) in hotkeyRefs {
            UnregisterEventHotKey(ref)
        }
        hotkeyRefs.removeAll(keepingCapacity: true)
        actionByID.removeAll(keepingCapacity: true)
    }

    private func handleHotkeyEvent(_ event: EventRef) {
        var hotkeyID = EventHotKeyID()
        let status = GetEventParameter(
            event,
            EventParamName(kEventParamDirectObject),
            EventParamType(typeEventHotKeyID),
            nil,
            MemoryLayout<EventHotKeyID>.size,
            nil,
            &hotkeyID
        )

        guard status == noErr else {
            return
        }

        lock.lock()
        let action = actionByID[hotkeyID.id]
        let handler = onHotkey
        lock.unlock()

        guard let action else {
            return
        }
        handler?(action)
    }

    private func carbonModifiers(from modifiers: HotkeyModifiers) -> UInt32 {
        var flags: UInt32 = 0
        if modifiers.contains(.command) {
            flags |= UInt32(cmdKey)
        }
        if modifiers.contains(.option) {
            flags |= UInt32(optionKey)
        }
        if modifiers.contains(.control) {
            flags |= UInt32(controlKey)
        }
        if modifiers.contains(.shift) {
            flags |= UInt32(shiftKey)
        }
        return flags
    }

    private func installWakeObserver() {
        wakeObserver = NotificationCenter.default.addObserver(
            forName: NSWorkspace.didWakeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard let self else {
                return
            }
            Task {
                await self.recoverAfterWake()
            }
        }
    }
}
