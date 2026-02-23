// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "WhisperAssistant",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "WhisperAssistantCore",
            targets: ["WhisperAssistantCore"]
        ),
        .executable(
            name: "WhisperAssistant",
            targets: ["WhisperAssistant"]
        ),
        .executable(
            name: "wa",
            targets: ["WhisperAssistantCLI"]
        )
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.7.0"),
        .package(url: "https://github.com/sparkle-project/Sparkle.git", from: "2.9.0")
    ],
    targets: [
        .target(
            name: "WhisperAssistantCore",
            dependencies: [],
            linkerSettings: [
                .linkedLibrary("sqlite3")
            ]
        ),
        .executableTarget(
            name: "WhisperAssistant",
            dependencies: [
                "WhisperAssistantCore",
                .product(name: "Sparkle", package: "Sparkle")
            ]
        ),
        .executableTarget(
            name: "WhisperAssistantCLI",
            dependencies: [
                "WhisperAssistantCore",
                .product(name: "ArgumentParser", package: "swift-argument-parser")
            ]
        ),
        .testTarget(
            name: "WhisperAssistantCoreTests",
            dependencies: ["WhisperAssistantCore"]
        )
    ]
)
