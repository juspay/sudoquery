// swift-tools-version: 6.0
// The SudoQuery iOS SDK: stores events on disk, batches them and delivers them to the
// collector's /v1/events/batch route.
import PackageDescription

let package = Package(
    name: "SudoQuery",
    // The SDK is for iOS apps. macOS is listed only so `swift test` runs on a Mac.
    platforms: [.iOS(.v13), .macOS(.v10_15)],
    products: [
        .library(name: "SudoQuery", targets: ["SudoQuery"]),
    ],
    targets: [
        .target(
            name: "SudoQuery",
            resources: [.copy("Resources/PrivacyInfo.xcprivacy")]
        ),
        .testTarget(
            name: "SudoQueryTests",
            dependencies: ["SudoQuery"],
            resources: [.copy("Resources/golden")]
        ),
    ]
)
