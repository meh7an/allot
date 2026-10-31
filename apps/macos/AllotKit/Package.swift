// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "AllotKit",
    platforms: [.macOS(.v14)],
    products: [
        .library(
            name: "AllotKit",
            targets: ["AllotKit"]
        )
    ],
    dependencies: [
        .package(path: "../../../contract/swift")
    ],
    targets: [
        .target(
            name: "AllotKit",
            dependencies: [
                .product(name: "AllotContract", package: "swift")
            ]
        ),
        .testTarget(
            name: "AllotKitTests",
            dependencies: [
                "AllotKit",
                .product(name: "AllotContract", package: "swift"),
            ]
        ),
    ]
)
