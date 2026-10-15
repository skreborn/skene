// swift-tools-version:5.9
import PackageDescription

let package = Package(
  name: "TextSpike",
  platforms: [.iOS(.v17)],
  products: [.library(name: "TextSpike", targets: ["TextSpike"])],
  targets: [
    .target(name: "TextSpike"),
    .testTarget(name: "TextSpikeTests", dependencies: ["TextSpike"]),
  ]
)
