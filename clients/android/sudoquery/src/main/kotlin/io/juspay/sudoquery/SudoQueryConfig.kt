package io.juspay.sudoquery

import java.net.MalformedURLException
import java.net.URL

/**
 * Configuration for [SudoQuery.init]. Create one with [Builder]:
 *
 * ```
 * val config = SudoQueryConfig.Builder(
 *     orgId = "acme-store-k3x9qa",
 *     projectId = "acme-store-app-9t2r4m",
 *     endpoint = "https://collector.example.com/v1/events/batch",
 * ).build()
 * ```
 */
public class SudoQueryConfig private constructor(
    internal val orgId: String,
    internal val projectId: String,
    internal val endpoint: String,
    internal val token: String?,
    internal val headers: Map<String, String>,
    internal val source: String?,
    internal val sessionId: String?,
    internal val batchSize: Int,
    internal val flushIntervalMillis: Long,
    internal val sessionTimeoutMillis: Long,
    internal val debugLogging: Boolean,
) {

    /**
     * @param orgId the organization id (a server-generated slug such as `acme-store-k3x9qa`),
     *   sent as `x-org-id` and in every event.
     * @param projectId the project id (a server-generated slug), sent as `x-project-id` and in
     *   every event.
     * @param endpoint the full URL of the collector's batch route, e.g.
     *   `https://collector.example.com/v1/events/batch`.
     */
    public class Builder(
        private val orgId: String,
        private val projectId: String,
        private val endpoint: String,
    ) {
        private var token: String? = null
        private var headers: Map<String, String> = emptyMap()
        private var source: String? = DEFAULT_SOURCE
        private var sessionId: String? = null
        private var batchSize: Int = DEFAULT_BATCH_SIZE
        private var flushIntervalMillis: Long = DEFAULT_FLUSH_INTERVAL_MILLIS
        private var sessionTimeoutMillis: Long = DEFAULT_SESSION_TIMEOUT_MILLIS
        private var debugLogging: Boolean = false

        /** Sent as `Authorization: Bearer <token>`, for a proxy or gateway in front of the collector. */
        public fun token(token: String?): Builder = apply { this.token = token }

        /**
         * Extra headers for every request. They can replace `Content-Type` and `User-Agent`,
         * but not `x-org-id`, `x-project-id` or the bearer [token].
         */
        public fun headers(headers: Map<String, String>): Builder = apply { this.headers = headers.toMap() }

        /** The `source` of every event. Defaults to `"android"`. */
        public fun source(source: String?): Builder = apply { this.source = source }

        /** Use this session id for every event instead of SDK-managed sessions. */
        public fun sessionId(sessionId: String?): Builder = apply { this.sessionId = sessionId }

        /** Events per batch file and per request, 1 to 500. Defaults to 50. */
        public fun batchSize(batchSize: Int): Builder = apply { this.batchSize = batchSize }

        /** How often pending events are sent even if a batch isn't full. Defaults to 30 s; 0 turns the timer off. */
        public fun flushIntervalMillis(millis: Long): Builder = apply { this.flushIntervalMillis = millis }

        /** A new session starts when no event was tracked for this long. Defaults to 30 minutes. */
        public fun sessionTimeoutMillis(millis: Long): Builder = apply { this.sessionTimeoutMillis = millis }

        /** Log debug details to logcat under the `SudoQuery` tag. Warnings and errors are always logged. */
        public fun debugLogging(enabled: Boolean): Builder = apply { this.debugLogging = enabled }

        /** @throws IllegalArgumentException if an id is blank, the endpoint isn't an http(s) URL, or a value is out of range. */
        public fun build(): SudoQueryConfig {
            require(orgId.isNotBlank()) { "orgId must not be blank" }
            require(projectId.isNotBlank()) { "projectId must not be blank" }
            val url = try {
                URL(endpoint)
            } catch (e: MalformedURLException) {
                throw IllegalArgumentException("endpoint is not a valid URL: $endpoint", e)
            }
            require(url.protocol == "https" || url.protocol == "http") {
                "endpoint must be an http(s) URL: $endpoint"
            }
            require(batchSize in 1..MAX_BATCH_SIZE) { "batchSize must be between 1 and $MAX_BATCH_SIZE, was $batchSize" }
            require(flushIntervalMillis >= 0) { "flushIntervalMillis must not be negative, was $flushIntervalMillis" }
            require(sessionTimeoutMillis > 0) { "sessionTimeoutMillis must be positive, was $sessionTimeoutMillis" }
            return SudoQueryConfig(
                orgId = orgId,
                projectId = projectId,
                endpoint = endpoint,
                token = token,
                headers = headers,
                source = source,
                sessionId = sessionId,
                batchSize = batchSize,
                flushIntervalMillis = flushIntervalMillis,
                sessionTimeoutMillis = sessionTimeoutMillis,
                debugLogging = debugLogging,
            )
        }
    }

    internal companion object {
        const val DEFAULT_SOURCE: String = "android"
        const val DEFAULT_BATCH_SIZE: Int = 50
        const val MAX_BATCH_SIZE: Int = 500
        const val DEFAULT_FLUSH_INTERVAL_MILLIS: Long = 30_000
        const val DEFAULT_SESSION_TIMEOUT_MILLIS: Long = 30 * 60 * 1000
    }
}
