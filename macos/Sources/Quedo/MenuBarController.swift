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
        enum IconStyle {
            case ready
            case recording
            case processing
        }

        let style: IconStyle?
        switch phase {
        case .ready:
            style = .ready
        case .recording:
            style = .recording
        case .processing, .streamingPartial, .providerFallback, .outputting:
            style = .processing
        default:
            style = nil
        }

        guard let style else {
            return nil
        }

        let size = NSSize(width: 22, height: 22)
        let image = NSImage(size: size, flipped: false) { _ in
            let center = NSPoint(x: 11, y: 11)

            NSColor.white.setFill()
            NSColor.white.setStroke()

            switch style {
            case .ready:
                let dotRadius: CGFloat = 2.0
                let dotRect = NSRect(
                    x: center.x - dotRadius, y: center.y - dotRadius,
                    width: dotRadius * 2, height: dotRadius * 2
                )
                NSBezierPath(ovalIn: dotRect).fill()

                let innerArc = NSBezierPath()
                innerArc.lineWidth = 1.5
                innerArc.appendArc(withCenter: center, radius: 4.5, startAngle: -60, endAngle: 60, clockwise: false)
                innerArc.stroke()

                let outerArc = NSBezierPath()
                outerArc.lineWidth = 1.5
                outerArc.appendArc(withCenter: center, radius: 7.0, startAngle: -60, endAngle: 60, clockwise: false)
                outerArc.stroke()

            case .recording:
                let dotRadius: CGFloat = 2.8
                let dotRect = NSRect(
                    x: center.x - dotRadius, y: center.y - dotRadius,
                    width: dotRadius * 2, height: dotRadius * 2
                )
                NSBezierPath(ovalIn: dotRect).fill()

                let innerArc = NSBezierPath()
                innerArc.lineWidth = 3.0
                innerArc.appendArc(withCenter: center, radius: 4.5, startAngle: -70, endAngle: 70, clockwise: false)
                innerArc.stroke()

                let outerArc = NSBezierPath()
                outerArc.lineWidth = 3.0
                outerArc.appendArc(withCenter: center, radius: 7.0, startAngle: -70, endAngle: 70, clockwise: false)
                outerArc.stroke()

            case .processing:
                let dotRadius: CGFloat = 2.0
                let dotRect = NSRect(
                    x: center.x - dotRadius, y: center.y - dotRadius,
                    width: dotRadius * 2, height: dotRadius * 2
                )
                NSBezierPath(ovalIn: dotRect).fill()

                let arc1 = NSBezierPath()
                arc1.lineWidth = 1.8
                arc1.appendArc(withCenter: center, radius: 3.5, startAngle: -45, endAngle: 45, clockwise: false)
                arc1.stroke()

                let arc2 = NSBezierPath()
                arc2.lineWidth = 1.8
                arc2.appendArc(withCenter: center, radius: 5.5, startAngle: -45, endAngle: 45, clockwise: false)
                arc2.stroke()

                let arc3 = NSBezierPath()
                arc3.lineWidth = 1.8
                arc3.appendArc(withCenter: center, radius: 7.5, startAngle: -45, endAngle: 45, clockwise: false)
                arc3.stroke()
            }

            return true
        }

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
