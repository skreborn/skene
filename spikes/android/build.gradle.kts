// AGP 9 compiles Kotlin itself (built-in Kotlin) on the Kotlin Gradle plugin it depends on; the
// classpath entry raises that plugin to the release the Compose compiler plugin matches.
buildscript {
  dependencies { classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.21") }
}

plugins {
  id("com.android.application") version "9.4.1" apply false
  id("org.jetbrains.kotlin.plugin.compose") version "2.4.21" apply false
}
