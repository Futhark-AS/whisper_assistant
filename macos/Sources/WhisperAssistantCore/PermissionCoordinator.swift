import AVFoundation
import AppKit
import ApplicationServices
import Foundation

/// Generic permission state.
public enum PermissionState: String, Codable, Sendable {
    /// Access is granted.
    case granted
    /// Access was denied.
    case denied
    /// Access has not been requested.
    case notDetermined
    /// Access is restricted by policy.
    case restricted
}

/// Snapshot of all relevant runtime permissions.
public struct PermissionSnapshot: Sendable {
    /// Microphone permission state.
    public let microphone: PermissionState
    /// Accessibility permission state.
    public let accessibility: PermissionState
    /// Input Monitoring state (used by compatibility mode only).
    public let inputMonitoring: PermissionState

    /// Creates a permission snapshot.
    public init(microphone: PermissionState, accessibility: PermissionState, inputMonitoring: PermissionState) {
        self.microphone = microphone
        self.accessibility = accessibility
        self.inputMonitoring = inputMonitoring
    }

    /// Returns true when all core permissions are granted.
    public var isFullyReady: Bool {
        microphone == .granted && accessibility == .granted
    }
}

/// Available System Settings destinations for remediation.
public enum SettingsPane: Sendable {
    /// Privacy microphone settings.
    case microphone
    /// Accessibility settings.
    case accessibility
    /// Input Monitoring settings.
    case inputMonitoring
}

/// Coordinates permission checks and remediation actions.
public actor PermissionCoordinator {
    /// Creates a permission coordinator.
    public init() {}

    /// Returns a complete permission snapshot.
    public func checkAll() -> PermissionSnapshot {
        PermissionSnapshot(
            microphone: microphoneStatus(),
            accessibility: accessibilityStatus(),
            inputMonitoring: inputMonitoringStatus()
        )
    }

    /// Requests microphone permission if not yet determined.
    public func requestMicrophonePermission() async -> PermissionState {
        let current = AVCaptureDevice.authorizationStatus(for: .audio)
        switch current {
        case .authorized:
            return .granted
        case .denied:
            return .denied
        case .restricted:
            return .restricted
        case .notDetermined:
            let granted = await AVCaptureDevice.requestAccess(for: .audio)
            return granted ? .granted : .denied
        @unknown default:
            return .restricted
        }
    }

    /// Opens System Settings at the requested pane.
    public func openSystemSettings(_ pane: SettingsPane) {
        let urlString: String
        switch pane {
        case .microphone:
            urlString = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        case .accessibility:
            urlString = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        case .inputMonitoring:
            urlString = "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }

        guard let url = URL(string: urlString) else {
            return
        }
        NSWorkspace.shared.open(url)
    }

    /// Requests Input Monitoring permission prompt when needed.
    public func requestInputMonitoringPrompt() {
        _ = CGRequestListenEventAccess()
    }

    private func microphoneStatus() -> PermissionState {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return .granted
        case .denied:
            return .denied
        case .notDetermined:
            return .notDetermined
        case .restricted:
            return .restricted
        @unknown default:
            return .restricted
        }
    }

    private func accessibilityStatus() -> PermissionState {
        let trusted = AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt as String: false] as CFDictionary)
        return trusted ? .granted : .denied
    }

    private func inputMonitoringStatus() -> PermissionState {
        let preflight = CGPreflightListenEventAccess()
        return preflight ? .granted : .denied
    }
}
