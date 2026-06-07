import org.gradle.api.tasks.bundling.Zip

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

val vendorAar = file("libs/obsensor_v2.0.6_2026031801_release.aar")
val slimAar = layout.buildDirectory.file(
    "generated/orbbec/obsensor_v2.0.6_arm64_no_firmware_updater.aar"
)
val prepareSlimOrbbecAar by tasks.registering(Zip::class) {
    from(zipTree(vendorAar))
    exclude("jni/armeabi-v7a/**")
    exclude("assets/armeabi-v7a/**")
    exclude("assets/arm64-v8a/extensions/firmwareupdater/**")
    archiveFileName.set(slimAar.get().asFile.name)
    destinationDirectory.set(slimAar.get().asFile.parentFile)
    isPreserveFileTimestamps = false
    isReproducibleFileOrder = true
}

android {
    namespace = "dev.sawitulm.palmannotate.rust.nativebridge"
    compileSdk = 34
    ndkVersion = "28.2.13676358"

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
        ndk { abiFilters += "arm64-v8a" }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
}

dependencies {
    implementation(files(slimAar).builtBy(prepareSlimOrbbecAar))
    implementation("com.microsoft.onnxruntime:onnxruntime-android:1.24.1")
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.documentfile:documentfile:1.0.1")
    implementation("androidx.camera:camera-core:1.4.2")
    implementation("androidx.camera:camera-camera2:1.4.2")
    implementation("androidx.camera:camera-lifecycle:1.4.2")
    implementation("androidx.camera:camera-view:1.4.2")
    implementation(project(":tauri-android"))
}
