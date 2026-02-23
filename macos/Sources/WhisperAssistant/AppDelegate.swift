import AppKit
import ServiceManagement
import Sparkle
import SwiftUI
import WhisperAssistantCore

/// Application delegate responsible for bootstrapping services and windows.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var menuBarController: MenuBarController?
    private var appController: AppControllerActor?

    private var preferencesWindowController: NSWindowController?
    private var historyWindowController: NSWindowController?
    private var updaterController: SPUStandardUpdaterController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        _ = notification

        let menuBar = MenuBarController()
        menuBarController = menuBar

        Task {
            await bootstrap(menuBar: menuBar)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        _ = notification
        Task {
            await appController?.shutdown()
        }
    }

    private func bootstrap(menuBar: MenuBarController) async {
        let configurationManager = ConfigurationManager()
        let permissionCoordinator = PermissionCoordinator()
        let lifecycle = LifecycleStateMachine()
        let audioEngine = AudioCaptureEngine()

        let historyStore: HistoryStore
        do {
            historyStore = try HistoryStore()
        } catch {
            presentFatalError(message: "History store initialization failed: \(error)")
            return
        }

        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Whisper Assistant", isDirectory: true)
        let fileLogger = RotatingFileLogger(directory: appSupport.appendingPathComponent("logs", isDirectory: true))
        let logger = AppLogger(subsystem: "com.whisperassistant.app", category: "runtime", fileLogger: fileLogger)
        let diagnostics = DiagnosticsCenter(historyStore: historyStore, logger: logger)

        let bootSettings = (try? await configurationManager.loadSettings()) ?? .default

        let groqProvider = GroqProvider(timeoutSeconds: bootSettings.provider.timeoutSeconds) {
            try await configurationManager.loadAPIKey(for: .groq) ?? ""
        }
        let openAIProvider = OpenAIProvider(timeoutSeconds: bootSettings.provider.timeoutSeconds) {
            try await configurationManager.loadAPIKey(for: .openAI) ?? ""
        }
        let transcriptionPipeline = TranscriptionPipeline(
            providers: [groqProvider, openAIProvider],
            requestTimeoutSeconds: bootSettings.provider.timeoutSeconds
        )

        let outputRouter = OutputRouter()
        let onboardingCoordinator = OnboardingCoordinator()

        let hotkeyManager: HotkeyManager
        do {
            hotkeyManager = try HotkeyManager()
        } catch {
            presentFatalError(message: "Hotkey manager initialization failed: \(error)")
            return
        }

        let controller = AppControllerActor(
            configurationManager: configurationManager,
            lifecycle: lifecycle,
            permissionCoordinator: permissionCoordinator,
            audioEngine: audioEngine,
            transcriptionPipeline: transcriptionPipeline,
            outputRouter: outputRouter,
            historyStore: historyStore,
            diagnostics: diagnostics,
            hotkeyManager: hotkeyManager,
            onboardingCoordinator: onboardingCoordinator
        ) { [weak self] snapshot, contract in
            self?.menuBarController?.update(snapshot: snapshot, contract: contract)
        }
        appController = controller

        menuBar.setActionHandler { [weak self] action in
            guard let self else {
                return
            }

            switch action {
            case .preferences, .openProviderSettings:
                self.showPreferences(configurationManager: configurationManager)
            case .history:
                self.showHistory(historyStore: historyStore)
            case .openSettings:
                Task {
                    let permissions = await permissionCoordinator.checkAll()
                    if permissions.microphone != .granted {
                        if permissions.microphone == .notDetermined {
                            _ = await permissionCoordinator.requestMicrophonePermission()
                        }
                        await permissionCoordinator.openSystemSettings(.microphone)
                        return
                    }
                    if permissions.accessibility != .granted {
                        await permissionCoordinator.requestAccessibilityPermissionPrompt()
                        await permissionCoordinator.openSystemSettings(.accessibility)
                        return
                    }
                    if permissions.inputMonitoring != .granted {
                        await permissionCoordinator.requestInputMonitoringPrompt()
                        await permissionCoordinator.openSystemSettings(.inputMonitoring)
                        return
                    }
                    await permissionCoordinator.openSystemSettings(.microphone)
                }
            case .selectDevice:
                self.openSoundInputSettings()
            case .viewLastError:
                Task {
                    let description = await controller.lastErrorDescription()
                    self.presentInfoAlert(title: "Last Error", message: description)
                }
            case .viewDiagnostics:
                Task {
                    await diagnostics.emit(
                        DiagnosticEvent(name: "open_diagnostics_requested", sessionID: nil, attributes: [:])
                    )
                }
            case .exportDiagnostics:
                Task {
                    do {
                        let archiveURL = try await diagnostics.exportDiagnosticsBundle()
                        self.revealDiagnosticsArchive(archiveURL)
                    } catch {
                        self.presentInfoAlert(
                            title: "Export Diagnostics Failed",
                            message: error.localizedDescription
                        )
                    }
                }
            default:
                Task {
                    await controller.handleMenuAction(action)
                }
            }
        }

        configureUpdaterIfSupported(settings: bootSettings)

        registerForLoginAtStartupIfEnabled(settings: bootSettings)

        await controller.boot()
    }

    private func showPreferences(configurationManager: ConfigurationManager) {
        let view = PreferencesView(configurationManager: configurationManager) { [weak self] in
            Task {
                await self?.appController?.reloadSettingsFromDisk()
                if let latestSettings = try? await configurationManager.loadSettings() {
                    self?.applyLaunchAtLoginPreference(settings: latestSettings)
                }
            }
        }
        let controller = NSHostingController(rootView: view)
        let window = NSWindow(contentViewController: controller)
        window.title = "Preferences"
        window.styleMask = [.titled, .closable, .resizable, .miniaturizable]
        window.setContentSize(NSSize(width: 620, height: 680))

        let windowController = NSWindowController(window: window)
        windowController.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
        preferencesWindowController = windowController
    }

    private func showHistory(historyStore: HistoryStore) {
        let view = HistoryView(historyStore: historyStore)
        let controller = NSHostingController(rootView: view)
        let window = NSWindow(contentViewController: controller)
        window.title = "History"
        window.styleMask = [.titled, .closable, .resizable, .miniaturizable]
        window.setContentSize(NSSize(width: 720, height: 460))

        let windowController = NSWindowController(window: window)
        windowController.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
        historyWindowController = windowController
    }

    private func registerForLoginAtStartupIfEnabled(settings: AppSettings) {
        applyLaunchAtLoginPreference(settings: settings)
    }

    private func applyLaunchAtLoginPreference(settings: AppSettings) {
        guard Bundle.main.bundlePath.hasSuffix(".app") else {
            return
        }

        do {
            if settings.launchAtLoginEnabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
        } catch {
            // Launch-at-login failures should not block launch.
        }
    }

    private func configureUpdaterIfSupported(settings: AppSettings) {
        guard settings.buildProfile == .direct else {
            return
        }
        guard Bundle.main.bundlePath.hasSuffix(".app") else {
            return
        }
        guard Bundle.main.object(forInfoDictionaryKey: "SUFeedURL") != nil else {
            return
        }

        updaterController = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
    }

    private func openSoundInputSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.sound?input") else {
            presentInfoAlert(title: "Unable to Open Settings", message: "Could not construct Sound settings URL.")
            return
        }
        NSWorkspace.shared.open(url)
    }

    private func revealDiagnosticsArchive(_ archiveURL: URL) {
        NSWorkspace.shared.activateFileViewerSelecting([archiveURL])
        presentInfoAlert(
            title: "Diagnostics Exported",
            message: archiveURL.path
        )
    }

    private func presentInfoAlert(title: String, message: String) {
        NSApp.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .informational
        alert.runModal()
    }

    private func presentFatalError(message: String) {
        NSApp.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.messageText = "Whisper Assistant failed to start"
        alert.informativeText = message
        alert.alertStyle = .critical
        alert.runModal()
        NSApplication.shared.terminate(nil)
    }
}
