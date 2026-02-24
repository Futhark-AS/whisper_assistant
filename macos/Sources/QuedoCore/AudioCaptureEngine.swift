import AVFoundation
import AppKit
import Foundation

/// Errors produced by capture operations.
public enum AudioCaptureError: Error, Sendable {
    /// Capture already active.
    case alreadyRecording
    /// Capture not active.
    case notRecording
    /// Unable to open capture stream.
    case streamOpenFailed
    /// No input device is available.
    case noInputDevice
    /// No frames were observed in watchdog window.
    case callbackStalled
    /// Stop operation timed out.
    case stopTimedOut
    /// Audio file writer failed.
    case writerFailed
}

/// Completed recording output.
public struct AudioCaptureResult: Sendable {
    /// Session identifier.
    public let sessionID: UUID
    /// Recorded file URL.
    public let fileURL: URL
    /// Duration in milliseconds.
    public let durationMS: Int

    /// Creates a recording result.
    public init(sessionID: UUID, fileURL: URL, durationMS: Int) {
        self.sessionID = sessionID
        self.fileURL = fileURL
        self.durationMS = durationMS
    }
}

/// AVAudioEngine-based capture service with bounded recovery policies.
public actor AudioCaptureEngine {
    private let fileManager: FileManager
    private let workingDirectory: URL
    private var engine: AVAudioEngine
    private var writer: AVAudioFile?
    private var sessionID: UUID?
    private var startedAt: Date?
    private var lastFrameAt: Date?
    private var watchdogTask: Task<Void, Never>?
    private var armingWatchdogTask: Task<Void, Never>?
    private var pendingError: AudioCaptureError?
    private var observers: [NSObjectProtocol] = []
    private var observersInstalled = false

    /// Creates capture engine.
    public init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
        self.engine = AVAudioEngine()
        self.workingDirectory = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("QuedoCapture", isDirectory: true)
    }

    /// Prepares the audio engine to reduce first-start latency.
    public func prepareEngine() {
        prepareEngineIfPossible()
    }

    /// Starts a recording session with retry and watchdog policies.
    public func startRecording(sessionID: UUID) async throws {
        ensureEnvironmentObserversInstalled()

        guard self.sessionID == nil else {
            throw AudioCaptureError.alreadyRecording
        }

        var lastError: Error?
        for attempt in 0..<2 {
            do {
                try setupAndStart(sessionID: sessionID)
                startWatchdogs()
                return
            } catch {
                lastError = error
                teardownEngine(force: true)
                if attempt == 0 {
                    try? await Task.sleep(for: .milliseconds(300))
                }
            }
        }

        if (lastError as? AudioCaptureError) == .noInputDevice {
            throw AudioCaptureError.noInputDevice
        }
        throw AudioCaptureError.streamOpenFailed
    }

    /// Waits until first audio frame is received for active session.
    public func waitForFirstFrame(timeout: Duration = .seconds(2)) async -> Bool {
        if lastFrameAt != nil {
            return true
        }

        let clock = ContinuousClock()
        let deadline = clock.now + timeout
        while clock.now < deadline {
            guard sessionID != nil else {
                return false
            }
            if pendingError != nil {
                return false
            }
            if lastFrameAt != nil {
                return true
            }
            try? await Task.sleep(for: .milliseconds(25))
        }

        return lastFrameAt != nil
    }

    /// Stops recording and finalizes the audio artifact.
    public func stopRecording() async throws -> AudioCaptureResult {
        guard let activeSessionID = sessionID, let startedAt else {
            throw AudioCaptureError.notRecording
        }

        let stopSucceeded = await stopWithWatchdog(timeout: .seconds(2))
        if !stopSucceeded {
            teardownEngine(force: true)
            throw AudioCaptureError.stopTimedOut
        }

        endWatchdogs()

        if let pendingError {
            self.pendingError = nil
            throw pendingError
        }

        writer = nil
        self.sessionID = nil

        let durationMS = Int(Date().timeIntervalSince(startedAt) * 1000)
        let path = workingDirectory.appendingPathComponent(activeSessionID.uuidString).appendingPathExtension("wav")
        return AudioCaptureResult(sessionID: activeSessionID, fileURL: path, durationMS: max(durationMS, 0))
    }

    /// Cancels recording and tears down resources.
    public func cancelRecording() {
        teardownEngine(force: true)
        endWatchdogs()
        sessionID = nil
        writer = nil
        pendingError = nil
    }

    private func ensureEnvironmentObserversInstalled() {
        guard !observersInstalled else {
            return
        }
        installEnvironmentObservers()
        observersInstalled = true
    }

    private func setupAndStart(sessionID: UUID) throws {
        let input = engine.inputNode
        let format = input.inputFormat(forBus: 0)

        if format.channelCount == 0 {
            throw AudioCaptureError.noInputDevice
        }

        try fileManager.createDirectory(at: workingDirectory, withIntermediateDirectories: true)
        let outputURL = workingDirectory.appendingPathComponent(sessionID.uuidString).appendingPathExtension("wav")
        if fileManager.fileExists(atPath: outputURL.path) {
            try fileManager.removeItem(at: outputURL)
        }

        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: format.sampleRate,
            AVNumberOfChannelsKey: Int(format.channelCount),
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false
        ]

        let outputFile = try AVAudioFile(forWriting: outputURL, settings: settings)
        writer = outputFile

        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            guard let self else {
                return
            }
            do {
                try outputFile.write(from: buffer)
                Task {
                    await self.markFrameReceived()
                }
            } catch {
                Task {
                    await self.markWriterFailure()
                }
            }
        }

        prepareEngineIfPossible()
        do {
            try engine.start()
        } catch {
            throw AudioCaptureError.streamOpenFailed
        }

        self.sessionID = sessionID
        self.startedAt = Date()
        self.lastFrameAt = nil
        self.pendingError = nil
    }

    private func stopWithWatchdog(timeout: Duration) async -> Bool {
        _ = timeout
        let start = Date()
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()

        let elapsedSeconds = Date().timeIntervalSince(start)
        return elapsedSeconds <= 2.0
    }

    private func startWatchdogs() {
        armingWatchdogTask?.cancel()
        armingWatchdogTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(1500))
            guard let self else {
                return
            }
            await self.handleArmingWatchdog()
        }

        watchdogTask?.cancel()
        watchdogTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(250))
                guard let self else {
                    return
                }
                await self.handleCallbackWatchdog()
            }
        }
    }

    private func endWatchdogs() {
        armingWatchdogTask?.cancel()
        armingWatchdogTask = nil
        watchdogTask?.cancel()
        watchdogTask = nil
    }

    private func handleArmingWatchdog() {
        guard sessionID != nil else {
            return
        }

        if lastFrameAt == nil {
            engine.stop()
            prepareEngineIfPossible()
            do {
                try engine.start()
            } catch {
                pendingError = .streamOpenFailed
            }
        }
    }

    private func handleCallbackWatchdog() {
        guard sessionID != nil else {
            return
        }

        guard let lastFrameAt else {
            return
        }

        if Date().timeIntervalSince(lastFrameAt) > 0.75 {
            pendingError = .callbackStalled
            teardownEngine(force: true)
        }
    }

    private func markFrameReceived() {
        lastFrameAt = Date()
    }

    private func markWriterFailure() {
        pendingError = .writerFailed
    }

    private func teardownEngine(force: Bool) {
        if force {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
            engine.reset()
            engine = AVAudioEngine()
        } else {
            engine.stop()
        }
    }

    private func installEnvironmentObservers() {
        let center = NotificationCenter.default
        let workspaceCenter = NSWorkspace.shared.notificationCenter

        let configObserver = center.addObserver(forName: .AVAudioEngineConfigurationChange, object: nil, queue: nil) { [weak self] _ in
            Task {
                await self?.handleRouteChange()
            }
        }
        observers.append(configObserver)

        let willSleep = workspaceCenter.addObserver(forName: NSWorkspace.willSleepNotification, object: nil, queue: nil) { [weak self] _ in
            Task {
                await self?.handleSystemSleep()
            }
        }
        observers.append(willSleep)

        let didWake = workspaceCenter.addObserver(forName: NSWorkspace.didWakeNotification, object: nil, queue: nil) { [weak self] _ in
            Task {
                await self?.handleSystemWake()
            }
        }
        observers.append(didWake)
    }

    private func handleRouteChange() {
        guard sessionID != nil else {
            return
        }
        pendingError = .streamOpenFailed
    }

    private func handleSystemSleep() {
        if sessionID != nil {
            pendingError = .streamOpenFailed
            teardownEngine(force: true)
        }
    }

    private func handleSystemWake() {
        prepareEngineIfPossible()
    }

    private func prepareEngineIfPossible() {
        let inputFormat = engine.inputNode.inputFormat(forBus: 0)
        guard inputFormat.channelCount > 0 else {
            return
        }
        engine.prepare()
    }
}
