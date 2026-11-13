buildscript {
    repositories {
        mavenCentral()
    }
    dependencies {
        // AGP 9's built-in Kotlin compiles with the Kotlin Gradle plugin on this classpath.
        // Pinning it keeps it in step with the serialization compiler plugin below.
        classpath(libs.kotlin.gradle.plugin)
    }
}

plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.kotlin.serialization) apply false
    alias(libs.plugins.vanniktech.publish) apply false
}
