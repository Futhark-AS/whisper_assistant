import AppKit
import SwiftUI
import QuedoCore

/// History window showing recent transcription sessions.
struct HistoryView: View {
    @StateObject private var model: HistoryViewModel

    init(historyStore: HistoryStore) {
        _model = StateObject(wrappedValue: HistoryViewModel(historyStore: historyStore))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Recent Sessions")
                    .font(.headline)
                Spacer()
                Button("Refresh") {
                    Task {
                        await model.load(reset: true)
                    }
                }
            }

            List(model.sessions, id: \.sessionID) { session in
                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .top, spacing: 12) {
                        Text(session.transcriptPreview)
                            .lineLimit(3)
                            .frame(maxWidth: .infinity, alignment: .leading)

                        Button("Copy") {
                            Task {
                                await model.copyTranscript(sessionID: session.sessionID)
                            }
                        }
                        .buttonStyle(.bordered)
                    }

                    HStack {
                        Text(session.createdAt.formatted(date: .numeric, time: .shortened))
                        Text("•")
                        Text("\(session.durationMS) ms")
                        Text("•")
                        Text(session.providerUsed.rawValue)
                        Text("•")
                        Text(session.status.rawValue)
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
                .onAppear {
                    Task {
                        await model.loadMoreIfNeeded(visibleSessionID: session.sessionID)
                    }
                }
            }

            if model.hasMore {
                HStack {
                    Spacer()
                    Button(model.isLoadingMore ? "Loading..." : "Show More") {
                        Task {
                            await model.loadMore()
                        }
                    }
                    .disabled(model.isLoadingMore)
                    Spacer()
                }
            }

            if let status = model.statusMessage {
                Text(status)
                    .foregroundStyle(.secondary)
                    .font(.caption)
            }

            if let error = model.errorMessage {
                Text(error)
                    .foregroundStyle(.red)
                    .font(.caption)
            }
        }
        .padding(16)
        .frame(minWidth: 560, minHeight: 360)
        .task {
            await model.load(reset: true)
        }
    }
}

@MainActor
final class HistoryViewModel: ObservableObject {
    @Published var sessions: [HistorySessionSummary] = []
    @Published var errorMessage: String?
    @Published var statusMessage: String?
    @Published var hasMore = false
    @Published var isLoadingMore = false

    private let historyStore: HistoryStore
    private let pageSize = 100
    private var currentLimit = 100

    init(historyStore: HistoryStore) {
        self.historyStore = historyStore
    }

    func load(reset: Bool) async {
        if reset {
            currentLimit = pageSize
        }
        if !reset {
            isLoadingMore = true
        }
        defer {
            if !reset {
                isLoadingMore = false
            }
        }

        do {
            let loaded = try await historyStore.listSessions(limit: currentLimit + 1)
            hasMore = loaded.count > currentLimit
            sessions = Array(loaded.prefix(currentLimit))
            errorMessage = nil
            if reset {
                statusMessage = nil
            }
        } catch {
            errorMessage = "Failed to load history"
            hasMore = false
        }
    }

    func loadMore() async {
        guard hasMore, !isLoadingMore else {
            return
        }

        currentLimit += pageSize
        await load(reset: false)
    }

    func loadMoreIfNeeded(visibleSessionID: UUID) async {
        guard
            hasMore,
            !isLoadingMore,
            sessions.last?.sessionID == visibleSessionID
        else {
            return
        }

        await loadMore()
    }

    func copyTranscript(sessionID: UUID) async {
        do {
            guard let transcript = try await historyStore.transcriptText(sessionID: sessionID) else {
                statusMessage = "No transcript found"
                return
            }

            let trimmed = transcript.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                statusMessage = "Transcript is empty"
                return
            }

            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(trimmed, forType: .string)
            statusMessage = "Transcript copied"
            errorMessage = nil
        } catch {
            errorMessage = "Failed to copy transcript"
        }
    }
}
