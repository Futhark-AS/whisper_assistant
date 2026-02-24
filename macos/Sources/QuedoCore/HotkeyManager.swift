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

/// Hybrid hotkey manager using Carbon plus NSEvent monitoring for Fn-based combos.
public final class HotkeyManager: @unchecked Sendable {
    private struct MonitoredHotkey: Hashable {
        let keyCode: UInt32
        let modifiers: HotkeyModifiers
    }

    private static let signature: OSType = 0x57415354 // WAST

    private var handlerRef: EventHandlerRef?
    private var hotkeyRefs: [UInt32: EventHotKeyRef] = [:]
    private var actionByID: [UInt32: String] = [:]
    private var monitoredBindings: [MonitoredHotkey: String] = [:]
    private var modifierOnlyCombos: Set<MonitoredHotkey> = []
    private var registeredBindings: [HotkeyBinding] = []
    private var onHotkey: (@Sendable (String) -> Void)?
    private var globalMonitor: Any?
    private var localMonitor: Any?
    private var globalFlagsMonitor: Any?
    private var localFlagsMonitor: Any?
    private var activeModifierOnlyCombos: Set<MonitoredHotkey> = []
    private var lastDispatchByMonitoredHotkey: [MonitoredHotkey: Date] = [:]
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
        updateBindingsSnapshot(bindings: bindings, handler: handler)

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
        let bindings = currentBindingsSnapshot()

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

        var seenCombos = Set<MonitoredHotkey>()
        for binding in bindings {
            let combo = MonitoredHotkey(keyCode: binding.keyCode, modifiers: normalizedModifiers(binding.modifiers))
            if seenCombos.contains(combo) {
                throw HotkeyError.conflict
            }
            seenCombos.insert(combo)
        }

        let carbonBindings = bindings.filter { !shouldUseEventMonitor(for: $0) }
        let monitored = bindings.filter { shouldUseEventMonitor(for: $0) }

        for (index, binding) in carbonBindings.enumerated() {
            var ref: EventHotKeyRef?
            let identifier = EventHotKeyID(signature: Self.signature, id: UInt32(index + 1))
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

            lock.lock()
            hotkeyRefs[identifier.id] = ref
            actionByID[identifier.id] = binding.actionID
            lock.unlock()
        }

        configureEventMonitors(bindings: monitored)
    }

    private func unregisterAll() {
        lock.lock()
        let refs = Array(hotkeyRefs.values)
        hotkeyRefs.removeAll(keepingCapacity: true)
        actionByID.removeAll(keepingCapacity: true)
        monitoredBindings.removeAll(keepingCapacity: true)
        modifierOnlyCombos.removeAll(keepingCapacity: true)
        activeModifierOnlyCombos.removeAll(keepingCapacity: true)
        lastDispatchByMonitoredHotkey.removeAll(keepingCapacity: true)
        lock.unlock()

        for ref in refs {
            UnregisterEventHotKey(ref)
        }

        removeEventMonitors()
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

        let (action, handler) = currentActionAndHandler(for: hotkeyID.id)

        guard let action else {
            return
        }
        handler?(action)
    }

    private func handleMonitoredKeyDownEvent(_ event: NSEvent) {
        guard !event.isARepeat else {
            return
        }

        let combo = MonitoredHotkey(
            keyCode: UInt32(event.keyCode),
            modifiers: normalizedModifiers(hotkeyModifiers(from: event.modifierFlags))
        )
        guard combo.keyCode != HotkeyBinding.modifiersOnlyKeyCode else {
            return
        }
        let (action, handler, shouldDispatch) = currentMonitoredActionAndHandler(for: combo)
        guard shouldDispatch, let action else {
            return
        }
        handler?(action)
    }

    private func handleMonitoredFlagsChangedEvent(_ event: NSEvent) {
        let currentModifiers = normalizedModifiers(hotkeyModifiers(from: event.modifierFlags))
        let combos = currentModifierOnlyCombosSnapshot()

        for combo in combos {
            if combo.modifiers == currentModifiers {
                let shouldDispatchOnEdge = markModifierOnlyComboEntered(combo)
                guard shouldDispatchOnEdge else {
                    continue
                }

                let (action, handler, shouldDispatch) = currentMonitoredActionAndHandler(for: combo)
                guard shouldDispatch, let action else {
                    continue
                }
                handler?(action)
            } else {
                markModifierOnlyComboExited(combo)
            }
        }
    }

    private func updateBindingsSnapshot(bindings: [HotkeyBinding], handler: @escaping @Sendable (String) -> Void) {
        lock.lock()
        defer { lock.unlock() }
        self.registeredBindings = bindings
        self.onHotkey = handler
    }

    private func currentBindingsSnapshot() -> [HotkeyBinding] {
        lock.lock()
        defer { lock.unlock() }
        return registeredBindings
    }

    private func currentActionAndHandler(for hotkeyID: UInt32) -> (String?, (@Sendable (String) -> Void)?) {
        lock.lock()
        defer { lock.unlock() }
        return (actionByID[hotkeyID], onHotkey)
    }

    private func currentMonitoredActionAndHandler(
        for combo: MonitoredHotkey
    ) -> (String?, (@Sendable (String) -> Void)?, Bool) {
        lock.lock()
        defer { lock.unlock() }

        guard let action = monitoredBindings[combo] else {
            return (nil, onHotkey, false)
        }

        let now = Date()
        if let previous = lastDispatchByMonitoredHotkey[combo], now.timeIntervalSince(previous) < 0.20 {
            return (nil, onHotkey, false)
        }
        lastDispatchByMonitoredHotkey[combo] = now
        return (action, onHotkey, true)
    }

    private func currentModifierOnlyCombosSnapshot() -> [MonitoredHotkey] {
        lock.lock()
        defer { lock.unlock() }
        return Array(modifierOnlyCombos)
    }

    private func markModifierOnlyComboEntered(_ combo: MonitoredHotkey) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if activeModifierOnlyCombos.contains(combo) {
            return false
        }
        activeModifierOnlyCombos.insert(combo)
        return true
    }

    private func markModifierOnlyComboExited(_ combo: MonitoredHotkey) {
        lock.lock()
        defer { lock.unlock() }
        activeModifierOnlyCombos.remove(combo)
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

    private func hotkeyModifiers(from flags: NSEvent.ModifierFlags) -> HotkeyModifiers {
        var modifiers: HotkeyModifiers = []
        if flags.contains(.command) {
            modifiers.insert(.command)
        }
        if flags.contains(.option) {
            modifiers.insert(.option)
        }
        if flags.contains(.control) {
            modifiers.insert(.control)
        }
        if flags.contains(.shift) {
            modifiers.insert(.shift)
        }
        if flags.contains(.function) {
            modifiers.insert(.function)
        }
        return modifiers
    }

    private func normalizedModifiers(_ modifiers: HotkeyModifiers) -> HotkeyModifiers {
        var result: HotkeyModifiers = []
        if modifiers.contains(.command) {
            result.insert(.command)
        }
        if modifiers.contains(.option) {
            result.insert(.option)
        }
        if modifiers.contains(.control) {
            result.insert(.control)
        }
        if modifiers.contains(.shift) {
            result.insert(.shift)
        }
        if modifiers.contains(.function) {
            result.insert(.function)
        }
        return result
    }

    private func shouldUseEventMonitor(for binding: HotkeyBinding) -> Bool {
        // Carbon RegisterEventHotKey only supports classic modifier bits and requires a concrete key.
        binding.modifiers.contains(.function) || binding.keyCode == HotkeyBinding.modifiersOnlyKeyCode
    }

    private func configureEventMonitors(bindings: [HotkeyBinding]) {
        guard !bindings.isEmpty else {
            lock.lock()
            monitoredBindings.removeAll(keepingCapacity: true)
            modifierOnlyCombos.removeAll(keepingCapacity: true)
            activeModifierOnlyCombos.removeAll(keepingCapacity: true)
            lastDispatchByMonitoredHotkey.removeAll(keepingCapacity: true)
            lock.unlock()
            removeEventMonitors()
            return
        }

        var mapped: [MonitoredHotkey: String] = [:]
        for binding in bindings {
            let combo = MonitoredHotkey(
                keyCode: binding.keyCode,
                modifiers: normalizedModifiers(binding.modifiers)
            )
            mapped[combo] = binding.actionID
        }

        lock.lock()
        monitoredBindings = mapped
        modifierOnlyCombos = Set(mapped.keys.filter { $0.keyCode == HotkeyBinding.modifiersOnlyKeyCode })
        activeModifierOnlyCombos.removeAll(keepingCapacity: true)
        lastDispatchByMonitoredHotkey.removeAll(keepingCapacity: true)
        lock.unlock()

        installEventMonitorsIfNeeded()
    }

    private func installEventMonitorsIfNeeded() {
        let install = { [weak self] in
            guard let self else {
                return
            }
            if self.globalMonitor == nil {
                self.globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { [weak self] event in
                    self?.handleMonitoredKeyDownEvent(event)
                }
            }
            if self.localMonitor == nil {
                self.localMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                    self?.handleMonitoredKeyDownEvent(event)
                    return event
                }
            }
            if self.globalFlagsMonitor == nil {
                self.globalFlagsMonitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
                    self?.handleMonitoredFlagsChangedEvent(event)
                }
            }
            if self.localFlagsMonitor == nil {
                self.localFlagsMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
                    self?.handleMonitoredFlagsChangedEvent(event)
                    return event
                }
            }
        }

        if Thread.isMainThread {
            install()
        } else {
            DispatchQueue.main.sync(execute: install)
        }
    }

    private func removeEventMonitors() {
        let remove = { [weak self] in
            guard let self else {
                return
            }
            if let globalMonitor {
                NSEvent.removeMonitor(globalMonitor)
                self.globalMonitor = nil
            }
            if let localMonitor {
                NSEvent.removeMonitor(localMonitor)
                self.localMonitor = nil
            }
            if let globalFlagsMonitor {
                NSEvent.removeMonitor(globalFlagsMonitor)
                self.globalFlagsMonitor = nil
            }
            if let localFlagsMonitor {
                NSEvent.removeMonitor(localFlagsMonitor)
                self.localFlagsMonitor = nil
            }
        }

        if Thread.isMainThread {
            remove()
        } else {
            DispatchQueue.main.sync(execute: remove)
        }
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
