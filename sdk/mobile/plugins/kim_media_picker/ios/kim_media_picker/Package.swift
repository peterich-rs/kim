// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "kim_media_picker",
    platforms: [
        .iOS("15.0")
    ],
    products: [
        .library(name: "kim-media-picker", targets: ["kim_media_picker"])
    ],
    dependencies: [
        .package(name: "FlutterFramework", path: "../FlutterFramework")
    ],
    targets: [
        .target(
            name: "kim_media_picker",
            dependencies: [
                .product(name: "FlutterFramework", package: "FlutterFramework")
            ],
            resources: [
                .process("PrivacyInfo.xcprivacy"),
            ]
        )
    ]
)
