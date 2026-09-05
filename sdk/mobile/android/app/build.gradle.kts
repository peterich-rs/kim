import groovy.json.JsonSlurper

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}


// Flutter version used to compile this APK — also the OTA engine_build_id.
// Sourced from sdk/mobile/.fvmrc (same file CI flutter-action consumes).
val otaEngineBuildId: String =
    run {
        val fvmrc = file("../../.fvmrc")
        require(fvmrc.isFile) { "Missing ${fvmrc.path}; cannot set OTA_ENGINE_BUILD_ID" }
        @Suppress("UNCHECKED_CAST")
        val map = JsonSlurper().parse(fvmrc) as Map<String, Any?>
        val v = map["flutter"] as? String
        require(!v.isNullOrBlank()) { "Could not parse flutter version from ${fvmrc.path}" }
        v!!
    }

android {
    namespace = "com.kim.kim_mobile"
    buildFeatures {
        buildConfig = true
    }
    // Flutter 3.47 still defaults compileSdk to 36. flutter_secure_storage 11
    // and permission_handler 13 compile against 37; Flutter's Android build
    // guide says bump compileSdk when a plugin needs a newer API:
    // https://docs.flutter.dev/deployment/android#reviewing-the-gradle-build-configuration
    compileSdk = 37
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "com.kim.kim_mobile"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
        // Ship arm64-v8a only. Fat multi-ABI APKs multiply Flutter engine +
        // Rust kim_client_ffi + sqlite3 native assets (~tens of MB). Modern
        // Android devices are 64-bit; drop armeabi-v7a / x86 / x86_64.
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
        // Logic SO OTA: host_line is derived at runtime from VERSION_NAME (x.y).
        // engine_build_id matches the Flutter version that compiled this APK.
        buildConfigField("String", "OTA_ENGINE_BUILD_ID", "\"${otaEngineBuildId}\"")
        buildConfigField("String", "OTA_CHANNEL", "\"dev\"")
        buildConfigField("String", "OTA_GITHUB_OWNER", "\"peterich-rs\"")
        buildConfigField("String", "OTA_GITHUB_REPO", "\"kim\"")
        buildConfigField("String", "OTA_TAG_PREFIX", "\"logic-ota-v\"")
    }

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signing with the debug keys for now, so `flutter run --release` works.
            signingConfig = signingConfigs.getByName("debug")
            // Flutter 3.47 enables R8 minify for release. Extra JNI/FRB and
            // Play Core dontwarn rules live in proguard-rules.pro.
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}


dependencies {
    // Ed25519 verify for logic SO OTA manifests (OpenSSL pkeyutl compatible).
    implementation("org.bouncycastle:bcprov-jdk18on:1.80")
}
