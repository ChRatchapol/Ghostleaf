// Ghostleaf for Android. The whole app is native Rust (libghostleaf.so, built with
// `cargo ndk` into src/main/jniLibs), started by the platform NativeActivity.
// A small Java activity (MainActivity) adds the file picker, "Open with" and system-bar colours.
plugins {
    id("com.android.application")
}

val versionFromCargo: String = run {
    val toml = rootProject.file("../Cargo.toml").readText()
    Regex("""(?m)^version\s*=\s*"([^"]+)"""").find(toml)?.groupValues?.get(1) ?: "0.0.0"
}
val versionCodeFromCargo: Int = versionFromCargo.split(".").map { it.toIntOrNull() ?: 0 }
    .let { (it.getOrElse(0) { 0 } * 10000) + (it.getOrElse(1) { 0 } * 100) + it.getOrElse(2) { 0 } }

android {
    namespace = "app.ghostleaf"
    compileSdk = 35
    

    defaultConfig {
        applicationId = "app.ghostleaf"
        minSdk = 26
        // 34, not 35: Android 15 forces apps targeting 35 to draw under the status
        // and navigation bars, and egui can't read those insets on Android yet.
        targetSdk = 34
        versionCode = versionCodeFromCargo
        versionName = versionFromCargo
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = false
            // Signing is configured in phase 5 (release keystore from CI secrets).
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    lint {
        // the app has no Java/Kotlin code yet; don't let lint block release packaging
        abortOnError = false
        checkReleaseBuilds = false
    }

    packaging {
        // keep .so files uncompressed + page-aligned (faster load, smaller install)
        jniLibs { useLegacyPackaging = false }
    }
}
