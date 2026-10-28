// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "AllotContract",
    platforms: [.macOS(.v14)],
    products: [
        .library(
            name: "AllotContract",
            targets: ["AllotContract"]
        )
    ],
    targets: [
        .target(name: "AllotContract"),
        .testTarget(
            name: "AllotContractTests",
            dependencies: ["AllotContract"]
        ),
    ]
)
