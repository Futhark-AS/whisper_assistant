import AppKit
import QuedoCore

/// Manages the NSStatusItem menu bar UI and action routing.
@MainActor
final class MenuBarController: NSObject {
    private let statusItem: NSStatusItem
    private var actionHandler: ((AppAction) -> Void)?
    private var lastSnapshot: AppLifecycleSnapshot?
    private var lastContract: UIStateContract?
    private var completionResetWorkItem: DispatchWorkItem?

    init(statusItem: NSStatusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)) {
        self.statusItem = statusItem
        super.init()
        configureStatusItemAppearance(iconName: "mic", phase: .ready)
    }

    func setActionHandler(_ handler: @escaping (AppAction) -> Void) {
        actionHandler = handler
    }

    func update(snapshot: AppLifecycleSnapshot, contract: UIStateContract) {
        let previousPhase = lastSnapshot?.phase
        lastSnapshot = snapshot
        lastContract = contract

        if previousPhase == .outputting && snapshot.phase == .ready {
            showCompletionIndicator()
        } else {
            completionResetWorkItem?.cancel()
            completionResetWorkItem = nil
            configureStatusItemAppearance(iconName: contract.icon, phase: snapshot.phase)
        }

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

    private func showCompletionIndicator() {
        completionResetWorkItem?.cancel()
        completionResetWorkItem = nil

        if let image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Transcription complete") {
            image.isTemplate = true
            statusItem.button?.image = image
            statusItem.button?.title = ""
        }
        NSSound(named: NSSound.Name("Hero"))?.play()

        let workItem = DispatchWorkItem { [weak self] in
            guard
                let self,
                let snapshot = self.lastSnapshot,
                snapshot.phase == .ready,
                let contract = self.lastContract
            else {
                return
            }
            self.configureStatusItemAppearance(iconName: contract.icon, phase: snapshot.phase)
        }
        completionResetWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.1, execute: workItem)
    }

    private func configureStatusItemAppearance(iconName: String, phase: AppPhase) {
        guard let button = statusItem.button else {
            return
        }

        if let image = quedoTemplateImage(for: phase) {
            image.isTemplate = true
            button.image = image
            button.title = ""
            return
        }

        if let image = NSImage(systemSymbolName: iconName, accessibilityDescription: "Quedo") {
            image.isTemplate = true
            button.image = image
            button.title = ""
        } else {
            button.image = nil
            button.title = "Q"
        }
    }

    private func quedoTemplateImage(for phase: AppPhase) -> NSImage? {
        let mappedState: (dotRadius: CGFloat, lineWidth: CGFloat)?
        switch phase {
        case .ready:
            mappedState = (dotRadius: 1.6, lineWidth: 1.4)
        case .recording:
            mappedState = (dotRadius: 2.3, lineWidth: 1.5)
        case .processing, .streamingPartial, .providerFallback, .outputting:
            mappedState = (dotRadius: 1.8, lineWidth: 1.8)
        default:
            mappedState = nil
        }

        guard let mappedState else {
            return nil
        }

        let image = NSImage(size: NSSize(width: 18, height: 18))
        image.lockFocus()
        defer { image.unlockFocus() }

        let center = NSPoint(x: 9, y: 9)
        let dotRect = NSRect(
            x: center.x - mappedState.dotRadius,
            y: center.y - mappedState.dotRadius,
            width: mappedState.dotRadius * 2,
            height: mappedState.dotRadius * 2
        )

        NSColor.white.setFill()
        NSColor.white.setStroke()
        NSBezierPath(ovalIn: dotRect).fill()

        let innerArc = NSBezierPath()
        innerArc.lineWidth = mappedState.lineWidth
        innerArc.appendArc(withCenter: center, radius: 3.6, startAngle: -38, endAngle: 38, clockwise: false)
        innerArc.stroke()

        let outerArc = NSBezierPath()
        outerArc.lineWidth = mappedState.lineWidth
        outerArc.appendArc(withCenter: center, radius: 5.8, startAngle: -38, endAngle: 38, clockwise: false)
        outerArc.stroke()

        return image
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
