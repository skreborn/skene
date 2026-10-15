import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
  id("com.android.application")
  id("org.jetbrains.kotlin.android")
  id("org.jetbrains.kotlin.plugin.compose")
}

android {
  namespace = "dev.skene.spike.text"
  compileSdk = 35

  defaultConfig {
    applicationId = "dev.skene.spike.text"
    minSdk = 29
    targetSdk = 35
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

kotlin {
  compilerOptions { jvmTarget.set(JvmTarget.JVM_17) }
}

dependencies {
  val bom = platform("androidx.compose:compose-bom:2025.06.01")
  implementation(bom)
  implementation("androidx.activity:activity-compose:1.10.1")
  implementation("androidx.compose.foundation:foundation")
  implementation("androidx.compose.ui:ui")

  androidTestImplementation(bom)
  androidTestImplementation("androidx.test:core:1.6.1")
  androidTestImplementation("androidx.test:runner:1.6.2")
  androidTestImplementation("androidx.test.ext:junit:1.2.1")
  androidTestImplementation("junit:junit:4.13.2")
}
