// swift-tools-version: 6.1
import PackageDescription

let package = Package(
    name: "Quedo",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "QuedoCore",
            targets: ["QuedoCore"]
        ),
        .executable(
            name: "Quedo",
            targets: ["Quedo"]
        ),
        .executable(
            name: "quedo-cli",
            targets: ["QuedoCLI"]
        )
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.7.0"),
        .package(url: "https://github.com/sparkle-project/Sparkle.git", from: "2.9.0")
    ],
    targets: [
        .target(
            name: "QuedoCore",
            dependencies: [],
            linkerSettings: [
                .linkedLibrary("sqlite3")
            ]
        ),
        .executableTarget(
            name: "Quedo",
            dependencies: [
                "QuedoCore",
                .product(name: "Sparkle", package: "Sparkle")
            ],
            resources: [
                .process("Assets.xcassets")
            ]
        ),
        .executableTarget(
            name: "QuedoCLI",
            dependencies: [
                "QuedoCore",
                .product(name: "ArgumentParser", package: "swift-argument-parser")
            ]
        ),
        .testTarget(
            name: "QuedoCoreTests",
            dependencies: ["QuedoCore"]
        )
    ]
)
