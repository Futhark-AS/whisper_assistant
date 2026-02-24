import Foundation
import OSLog

/// Supported logging levels.
public enum LogLevel: String, Codable, Sendable {
    /// Debug details.
    case debug
    /// Informational event.
    case info
    /// Warning condition.
    case warning
    /// Recoverable error.
    case error
}

/// Structured log payload.
public struct LogEntry: Codable, Sendable {
    /// Timestamp in UTC.
    public let timestamp: Date
    /// Log category.
    public let category: String
    /// Level.
    public let level: LogLevel
    /// Message body.
    public let message: String
    /// Optional metadata map.
    public let metadata: [String: String]

    /// Creates a log entry.
    public init(timestamp: Date = Date(), category: String, level: LogLevel, message: String, metadata: [String: String] = [:]) {
        self.timestamp = timestamp
        self.category = category
        self.level = level
        self.message = message
        self.metadata = metadata
    }
}

/// Rotating file logger with size-based rollover.
public actor RotatingFileLogger {
    private let directory: URL
    private let fileManager: FileManager
    private let maxBytes: Int
    private let maxFiles: Int
    private let encoder = JSONEncoder()

    /// Creates file logger for a directory.
    public init(directory: URL, maxBytes: Int = 2_000_000, maxFiles: Int = 5, fileManager: FileManager = .default) {
        self.directory = directory
        self.fileManager = fileManager
        self.maxBytes = maxBytes
        self.maxFiles = maxFiles
        encoder.dateEncodingStrategy = .iso8601
    }

    /// Appends an entry and rotates if size threshold is exceeded.
    public func append(_ entry: LogEntry) {
        do {
            try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
            let active = directory.appendingPathComponent("app.log")
            if !fileManager.fileExists(atPath: active.path) {
                fileManager.createFile(atPath: active.path, contents: nil)
            }

            let data = try encoder.encode(entry)
            guard let handle = try? FileHandle(forWritingTo: active) else {
                return
            }
            defer {
                try? handle.close()
            }
            try handle.seekToEnd()
            try handle.write(contentsOf: data)
            try handle.write(contentsOf: Data([0x0A]))

            let attrs = try fileManager.attributesOfItem(atPath: active.path)
            let size = attrs[.size] as? NSNumber
            if size?.intValue ?? 0 > maxBytes {
                try rotate()
            }
        } catch {
            // File logging should never crash runtime.
        }
    }

    /// Returns path for currently active log file.
    public func activeLogPath() -> URL {
        directory.appendingPathComponent("app.log")
    }

    private func rotate() throws {
        for index in stride(from: maxFiles - 1, through: 1, by: -1) {
            let current = directory.appendingPathComponent("app.\(index).log")
            let next = directory.appendingPathComponent("app.\(index + 1).log")
            if fileManager.fileExists(atPath: next.path) {
                try fileManager.removeItem(at: next)
            }
            if fileManager.fileExists(atPath: current.path) {
                try fileManager.moveItem(at: current, to: next)
            }
        }

        let active = directory.appendingPathComponent("app.log")
        let first = directory.appendingPathComponent("app.1.log")
        if fileManager.fileExists(atPath: first.path) {
            try fileManager.removeItem(at: first)
        }
        if fileManager.fileExists(atPath: active.path) {
            try fileManager.moveItem(at: active, to: first)
        }
        fileManager.createFile(atPath: active.path, contents: nil)
    }
}

/// Unified logger that writes to OSLog and rotating files.
public actor AppLogger {
    private let osLogger: Logger
    private let category: String
    private let fileLogger: RotatingFileLogger

    /// Creates logger with subsystem and category.
    public init(subsystem: String, category: String, fileLogger: RotatingFileLogger) {
        self.osLogger = Logger(subsystem: subsystem, category: category)
        self.category = category
        self.fileLogger = fileLogger
    }

    /// Writes a log entry to both OSLog and file sink.
    public func log(_ level: LogLevel, _ message: String, metadata: [String: String] = [:]) async {
        switch level {
        case .debug:
            osLogger.debug("\(message, privacy: .public)")
        case .info:
            osLogger.info("\(message, privacy: .public)")
        case .warning:
            osLogger.warning("\(message, privacy: .public)")
        case .error:
            osLogger.error("\(message, privacy: .public)")
        }

        let entry = LogEntry(category: category, level: level, message: message, metadata: metadata)
        await fileLogger.append(entry)
    }

    /// Returns path to the active rotating log file.
    public func activeLogPath() async -> URL {
        await fileLogger.activeLogPath()
    }
}
