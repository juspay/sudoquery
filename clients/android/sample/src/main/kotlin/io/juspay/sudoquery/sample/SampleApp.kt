package io.juspay.sudoquery.sample

import android.app.Application
import io.juspay.sudoquery.SudoQuery
import io.juspay.sudoquery.SudoQueryConfig

class SampleApp : Application() {
    override fun onCreate() {
        super.onCreate()
        val config = SudoQueryConfig.Builder(
            orgId = BuildConfig.SUDOQUERY_ORG_ID,
            projectId = BuildConfig.SUDOQUERY_PROJECT_ID,
            endpoint = BuildConfig.SUDOQUERY_ENDPOINT,
        )
            .batchSize(20)
            .flushIntervalMillis(10_000)
            .debugLogging(true)
            .build()
        SudoQuery.init(this, config)
        SudoQuery.setSuperProperty("app_version", BuildConfig.VERSION_NAME)
    }
}
