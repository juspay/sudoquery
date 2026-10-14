import com.vanniktech.maven.publish.AndroidSingleVariantLibrary
import com.vanniktech.maven.publish.JavadocJar
import com.vanniktech.maven.publish.SourcesJar
import org.jetbrains.kotlin.gradle.dsl.KotlinVersion

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.vanniktech.publish)
}

val sdkVersion: String = providers.gradleProperty("VERSION_NAME").get()

android {
    namespace = "io.juspay.sudoquery"
    compileSdk = 36

    defaultConfig {
        minSdk = 21
        consumerProguardFiles("consumer-rules.pro")
        buildConfigField("String", "SDK_VERSION", "\"$sdkVersion\"")
        // Apps only need compileSdk 34 to consume the SDK, not this module's compileSdk.
        aarMetadata {
            minCompileSdk = 34
        }
    }

    buildFeatures {
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }

    lint {
        // Versions are pinned on purpose (see gradle/libs.versions.toml): newer ones would raise
        // the Kotlin, minSdk or Android Studio versions the SDK and this build require.
        disable += setOf("AndroidGradlePluginVersion", "GradleDependency", "NewerVersionAvailable")
    }
}

kotlin {
    explicitApi()
    // Compile against the Kotlin 2.1 stdlib and language so apps on Kotlin 2.0+ can consume the AAR.
    coreLibrariesVersion = "2.1.21"
    compilerOptions {
        languageVersion = KotlinVersion.KOTLIN_2_1
        apiVersion = KotlinVersion.KOTLIN_2_1
    }
}

dependencies {
    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.androidx.lifecycle.process)

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.okhttp.mockwebserver3)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)
    testImplementation(libs.org.json)
}

tasks.withType<Test>().configureEach {
    // Opt-in end-to-end test against a running collector:
    // ./gradlew :sudoquery:testDebugUnitTest -Psudoquery.liveCollectorUrl=http://localhost:3001/v1/events/batch
    systemProperty(
        "sudoquery.liveCollectorUrl",
        providers.gradleProperty("sudoquery.liveCollectorUrl").getOrElse(""),
    )
}

mavenPublishing {
    // Maven Central requires a javadoc jar; an empty one is accepted.
    configure(AndroidSingleVariantLibrary(javadocJar = JavadocJar.Empty(), sourcesJar = SourcesJar.Sources(), variant = "release"))
    coordinates(artifactId = "sudoquery-android")
    publishToMavenCentral()
    // CI passes the key as ORG_GRADLE_PROJECT_signingInMemoryKey; without it (local builds,
    // publishToMavenLocal) publications are left unsigned.
    if (providers.gradleProperty("signingInMemoryKey").isPresent) {
        signAllPublications()
    }
}
