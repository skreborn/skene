// swift-tools-version:5.9
import Foundation
import PackageDescription

// The Rust half (spikes/apple/parity) is built for the simulator before the package:
// `cargo build --release --lib --target aarch64-apple-ios-sim` in spikes/apple/parity.
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let rust = root.appendingPathComponent("../parity/target/aarch64-apple-ios-sim/release").standardized.path

let package = Package(
  name: "TextSpike",
  platforms: [.iOS(.v17)],
  products: [.library(name: "TextSpike", targets: ["TextSpike"])],
  targets: [
    .target(name: "TextSpike"),
    .target(
      name: "TextParityFFI",
      linkerSettings: [
        .unsafeFlags(["-L", rust]),
        .linkedLibrary("apple_parity"),
        .linkedLibrary("iconv"),
        .linkedLibrary("objc"),
        .linkedFramework("CoreFoundation"),
        .linkedFramework("CoreGraphics"),
        .linkedFramework("CoreText"),
        .linkedFramework("Foundation"),
        .linkedFramework("QuartzCore"),
        .linkedFramework("UIKit"),
      ]
    ),
    .testTarget(name: "TextSpikeTests", dependencies: ["TextSpike", "TextParityFFI"]),
  ]
)
