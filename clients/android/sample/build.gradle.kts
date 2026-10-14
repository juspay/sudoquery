plugins {
    alias(libs.plugins.android.application)
}

// Point the sample at a collector with -Psudoquery.endpoint=... (10.0.2.2 is the host machine from the emulator).
val endpoint = providers.gradleProperty("sudoquery.endpoint").getOrElse("http://10.0.2.2:3000/v1/events/batch")
val orgId = providers.gradleProperty("sudoquery.orgId").getOrElse("acme-k3x9qa")
val projectId = providers.gradleProperty("sudoquery.projectId").getOrElse("android-demo1")

android {
    namespace = "io.juspay.sudoquery.sample"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.juspay.sudoquery.sample"
        minSdk = 21
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
        buildConfigField("String", "SUDOQUERY_ENDPOINT", "\"$endpoint\"")
        buildConfigField("String", "SUDOQUERY_ORG_ID", "\"$orgId\"")
        buildConfigField("String", "SUDOQUERY_PROJECT_ID", "\"$projectId\"")
    }

    buildFeatures {
        buildConfig = true
    }

    buildTypes {
        // Minified like a production app, to check the SDK works under R8.
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

dependencies {
    implementation(project(":sudoquery"))
}
