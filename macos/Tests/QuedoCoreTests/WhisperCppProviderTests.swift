import Foundation
import XCTest
@testable import QuedoCore

final class WhisperCppProviderTests: XCTestCase {
    func testWhisperCppProviderReadsTranscriptFromOutputFile() async throws {
        let workspace = try makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace) }

        let scriptURL = try writeStubWhisperCLI(in: workspace)
        let modelURL = workspace.appendingPathComponent("model.bin")
        try Data("dummy".utf8).write(to: modelURL)

        let audioURL = workspace.appendingPathComponent("input.wav")
        try Data().write(to: audioURL)

        let provider = WhisperCppProvider(timeoutSeconds: 10, executablePath: scriptURL.path)
        let request = TranscriptionRequest(
            audioFileURL: audioURL,
            language: "auto",
            model: modelURL.path,
            context: nil,
            vocabularyHints: []
        )

        let response = try await provider.transcribe(request: request)
        XCTAssertEqual(response.provider, .whisperCpp)
        XCTAssertEqual(response.text, "stub transcript")
    }

    func testWhisperCppProviderThrowsTerminalWhenModelMissing() async throws {
        let workspace = try makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace) }

        let scriptURL = try writeStubWhisperCLI(in: workspace)
        let audioURL = workspace.appendingPathComponent("input.wav")
        try Data().write(to: audioURL)
        let missingModel = workspace.appendingPathComponent("missing-model.bin")

        let provider = WhisperCppProvider(timeoutSeconds: 10, executablePath: scriptURL.path)
        let request = TranscriptionRequest(
            audioFileURL: audioURL,
            language: "auto",
            model: missingModel.path,
            context: nil,
            vocabularyHints: []
        )

        do {
            _ = try await provider.transcribe(request: request)
            XCTFail("Expected terminal error")
        } catch let error as ProviderError {
            switch error {
            case .terminal:
                break
            default:
                XCTFail("Expected terminal error, got \(error)")
            }
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    private func makeWorkspace() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("quedo-whispercpp-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func writeStubWhisperCLI(in directory: URL) throws -> URL {
        let scriptURL = directory.appendingPathComponent("whisper-cli-stub.sh")
        let script = """
        #!/bin/sh
        set -eu
        out_prefix=""
        while [ "$#" -gt 0 ]; do
          case "$1" in
            -of)
              shift
              out_prefix="$1"
              ;;
          esac
          shift
        done
        printf "stub transcript" > "${out_prefix}.txt"
        """
        try script.write(to: scriptURL, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: scriptURL.path)
        return scriptURL
    }
}
