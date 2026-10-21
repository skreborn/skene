plugins {
  id("com.android.application")
  id("org.jetbrains.kotlin.plugin.compose")
}

android {
  namespace = "dev.skene.spike.text"
  compileSdk { version = release(37) { minorApiLevel = 2 } }
  buildToolsVersion = "37.0.0"
  // The NDK the Rust libraries are built with, which strips them here too.
  ndkVersion = "30.0.16248370"

  defaultConfig {
    applicationId = "dev.skene.spike.text"
    minSdk = 29
    targetSdk = 37
    versionCode = 1
    versionName = "1"
    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    ndk { abiFilters += listOf("x86_64") }
  }

  // Timings come from a non-debuggable build, as Compose's own benchmarks require.
  buildTypes {
    release {
      isMinifyEnabled = false
      signingConfig = signingConfigs.getByName("debug")
    }
  }
  testBuildType = "release"

  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }

  buildFeatures { compose = true }
  lint { checkReleaseBuilds = false }
}

dependencies {
  val bom = platform("androidx.compose:compose-bom:2026.09.00")
  implementation(bom)
  implementation("androidx.activity:activity-compose:1.13.0")
  implementation("androidx.compose.foundation:foundation")
  implementation("androidx.compose.ui:ui")

  androidTestImplementation(bom)
  androidTestImplementation("androidx.test:core:1.7.0")
  androidTestImplementation("androidx.test:runner:1.7.0")
  androidTestImplementation("androidx.test.ext:junit:1.3.0")
  androidTestImplementation("junit:junit:4.13.2")
}
