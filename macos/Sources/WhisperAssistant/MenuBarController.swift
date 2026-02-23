import AppKit
import WhisperAssistantCore

/// Manages the NSStatusItem menu bar UI and action routing.
@MainActor
final class MenuBarController: NSObject {
    private let statusItem: NSStatusItem
    private var actionHandler: ((AppAction) -> Void)?
    private var lastSnapshot: AppLifecycleSnapshot?
    private var lastContract: UIStateContract?

    init(statusItem: NSStatusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)) {
        self.statusItem = statusItem
        super.init()
        configureStatusItemAppearance(iconName: "mic")
    }

    func setActionHandler(_ handler: @escaping (AppAction) -> Void) {
        actionHandler = handler
    }

    func update(snapshot: AppLifecycleSnapshot, contract: UIStateContract) {
        lastSnapshot = snapshot
        lastContract = contract

        configureStatusItemAppearance(iconName: contract.icon)

        let menu = NSMenu()

        if let copy = contract.notificationCopy {
            let status = NSMenuItem(title: copy, action: nil, keyEquivalent: "")
            status.isEnabled = false
            menu.addItem(status)
            menu.addItem(.separator())
        }

        for action in contract.actions {
            menu.addItem(makeActionItem(for: action))
        }

        if snapshot.phase != .ready {
            if snapshot.currentSessionID != nil {
                menu.addItem(makeActionItem(for: .forceStop))
            }
            menu.addItem(makeActionItem(for: .viewLastError))
            menu.addItem(makeActionItem(for: .exportDiagnostics))
        }

        menu.addItem(.separator())
        menu.addItem(makeActionItem(for: .quit))

        statusItem.menu = menu
    }

    private func configureStatusItemAppearance(iconName: String) {
        guard let button = statusItem.button else {
            return
        }

        if let image = NSImage(systemSymbolName: iconName, accessibilityDescription: "Whisper Assistant") {
            image.isTemplate = true
            button.image = image
            button.title = ""
        } else {
            button.image = nil
            button.title = "WA"
        }
    }

    private func makeActionItem(for action: AppAction) -> NSMenuItem {
        let item = NSMenuItem(title: action.rawValue, action: #selector(handleMenuAction(_:)), keyEquivalent: "")
        item.target = self
        item.representedObject = action.rawValue
        if action == .quit {
            item.keyEquivalent = "q"
        }
        return item
    }

    @objc
    private func handleMenuAction(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let action = AppAction(rawValue: raw) else {
            return
        }

        if action == .quit {
            NSApplication.shared.terminate(nil)
            return
        }

        actionHandler?(action)
    }
}
