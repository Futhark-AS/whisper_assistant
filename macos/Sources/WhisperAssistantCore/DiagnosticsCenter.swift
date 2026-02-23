import Foundation

/// Diagnostic event payload.
public struct DiagnosticEvent: Sendable {
    /// Event name.
    public let name: String
    /// Optional session identifier.
    public let sessionID: UUID?
    /// Event attributes.
    public let attributes: [String: String]
    /// Event timestamp.
    public let timestamp: Date

    /// Creates a diagnostic event.
    public init(name: String, sessionID: UUID?, attributes: [String: String], timestamp: Date = Date()) {
        self.name = name
        self.sessionID = sessionID
        self.attributes = attributes
        self.timestamp = timestamp
    }
}

/// Metric value type for telemetry.
public struct MetricPoint: Sendable {
    /// Metric name.
    public let name: String
    /// Numeric value.
    public let value: Double
    /// Dimensions.
    public let tags: [String: String]
    /// Timestamp.
    public let timestamp: Date

    /// Creates a metric point.
    public init(name: String, value: Double, tags: [String: String], timestamp: Date = Date()) {
        self.name = name
        self.value = value
        self.tags = tags
        self.timestamp = timestamp
    }
}

/// Subsystems tracked by global recovery budget.
public enum RecoverySubsystem: String, Sendable {
    /// Audio capture subsystem.
    case audio
    /// Hotkey subsystem.
    case hotkey
    /// Provider subsystem.
    case provider
    /// Output subsystem.
    case output
}

/// Central diagnostics and telemetry manager.
public actor DiagnosticsCenter {
    private let historyStore: HistoryStore
    private let logger: AppLogger
    private let uploadEndpoint: URL?

    private var recoveryAttempts: [RecoverySubsystem: [Date]] = [:]
    private var counterRollup: [String: Double] = [:]
    private var rollupTask: Task<Void, Never>?
    private var uploadTask: Task<Void, Never>?

    /// Creates diagnostics center.
    public init(historyStore: HistoryStore, logger: AppLogger, uploadEndpoint: URL? = nil) {
        self.historyStore = historyStore
        self.logger = logger
        self.uploadEndpoint = uploadEndpoint
        startRollupLoop()
    }

    /// Emits a structured event entry.
    public func emit(_ event: DiagnosticEvent) async {
        do {
            try await historyStore.appendEvent(
                sessionID: event.sessionID,
                eventName: event.name,
                payload: event.attributes.merging([
                    "timestamp": ISO8601DateFormatter().string(from: event.timestamp)
                ]) { current, _ in current }
            )
        } catch {
            await logger.log(.error, "Failed to append diagnostic event", metadata: ["event": event.name])
        }

        await logger.log(.info, event.name, metadata: event.attributes)
    }

    /// Records a metric point and updates in-memory rollup.
    public func recordMetric(_ point: MetricPoint) async {
        let key = metricKey(name: point.name, tags: point.tags)
        counterRollup[key, default: 0] += point.value

        await emit(
            DiagnosticEvent(
                name: "metric_\(point.name)",
                sessionID: nil,
                attributes: point.tags.merging([
                    "value": String(point.value),
                    "timestamp": ISO8601DateFormatter().string(from: point.timestamp)
                ]) { current, _ in current }
            )
        )
    }

    /// Checks whether automatic recovery is still allowed for a subsystem.
    public func canAttemptRecovery(subsystem: RecoverySubsystem) -> Bool {
        pruneRecoveryWindow()
        let attempts = recoveryAttempts[subsystem] ?? []
        return attempts.count < 5
    }

    /// Records an automatic recovery attempt and result.
    public func recordRecoveryAttempt(subsystem: RecoverySubsystem, reason: String, success: Bool) async {
        pruneRecoveryWindow()
        recoveryAttempts[subsystem, default: []].append(Date())

        await recordMetric(
            MetricPoint(
                name: "recovery_attempt_total",
                value: 1,
                tags: [
                    "subsystem": subsystem.rawValue,
                    "reason": reason
                ]
            )
        )

        if success {
            await recordMetric(
                MetricPoint(
                    name: "recovery_success_total",
                    value: 1,
                    tags: [
                        "subsystem": subsystem.rawValue,
                        "reason": reason
                    ]
                )
            )
        }
    }

    /// Starts optional periodic upload loop for aggregated telemetry.
    public func startUploadLoop(optedIn: Bool) {
        uploadTask?.cancel()
        guard optedIn, uploadEndpoint != nil else {
            return
        }

        uploadTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(900))
                guard let self else {
                    return
                }
                await self.uploadRollup()
            }
        }
    }

    /// Exports diagnostics bundle and returns archive URL.
    public func exportDiagnosticsBundle() async throws -> URL {
        let root = await historyStore.storageBasePath()
        let exports = root.appendingPathComponent("exports", isDirectory: true)
        try FileManager.default.createDirectory(at: exports, withIntermediateDirectories: true)

        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        let archive = exports.appendingPathComponent("diagnostics-\(formatter.string(from: Date())).zip")

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.arguments = ["-r", archive.path, "db", "logs"]
        process.currentDirectoryURL = root

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                return archive
            }
            throw NSError(domain: "DiagnosticsCenter", code: Int(process.terminationStatus), userInfo: [NSLocalizedDescriptionKey: "zip command failed"])
        } catch {
            throw error
        }
    }

    private func pruneRecoveryWindow() {
        let cutoff = Date().addingTimeInterval(-600)
        for key in recoveryAttempts.keys {
            let filtered = recoveryAttempts[key, default: []].filter { $0 >= cutoff }
            recoveryAttempts[key] = filtered
        }
    }

    private func metricKey(name: String, tags: [String: String]) -> String {
        let suffix = tags.keys.sorted().map { "\($0)=\(tags[$0] ?? "")" }.joined(separator: ",")
        return "\(name){\(suffix)}"
    }

    private func startRollupLoop() {
        rollupTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(60))
                guard let self else {
                    return
                }
                await self.flushRollup()
            }
        }
    }

    private func flushRollup() async {
        guard !counterRollup.isEmpty else {
            return
        }

        for (key, value) in counterRollup {
            await logger.log(.info, "metrics_rollup_1m", metadata: ["metric": key, "value": String(value)])
        }

        counterRollup.removeAll(keepingCapacity: true)
    }

    private func uploadRollup() async {
        guard let endpoint = uploadEndpoint else {
            return
        }

        let payload: [String: Any] = [
            "timestamp": ISO8601DateFormatter().string(from: Date()),
            "metrics": counterRollup
        ]

        guard let body = try? JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys]) else {
            return
        }

        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body

        let delays: [Duration] = [.seconds(1), .seconds(5), .seconds(30)]

        for delay in delays {
            do {
                let (_, response) = try await URLSession.shared.data(for: request)
                if let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) {
                    return
                }
            } catch {
                await logger.log(.warning, "telemetry_upload_failed", metadata: ["error": String(describing: error)])
            }
            try? await Task.sleep(for: delay)
        }
    }
}
