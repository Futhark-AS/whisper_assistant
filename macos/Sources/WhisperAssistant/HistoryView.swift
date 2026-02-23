import SwiftUI
import WhisperAssistantCore

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
                        await model.load()
                    }
                }
            }

            List(model.sessions, id: \.sessionID) { session in
                VStack(alignment: .leading, spacing: 4) {
                    Text(session.transcriptPreview)
                        .lineLimit(2)
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
            await model.load()
        }
    }
}

@MainActor
final class HistoryViewModel: ObservableObject {
    @Published var sessions: [HistorySessionSummary] = []
    @Published var errorMessage: String?

    private let historyStore: HistoryStore

    init(historyStore: HistoryStore) {
        self.historyStore = historyStore
    }

    func load() async {
        do {
            sessions = try await historyStore.listSessions(limit: 200)
            errorMessage = nil
        } catch {
            errorMessage = "Failed to load history"
        }
    }
}
