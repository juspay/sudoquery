package io.juspay.sudoquery.internal

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL

internal class HttpResult(
    val code: Int,
    /** `Retry-After` in seconds, when the server sent one. */
    val retryAfterSeconds: Long?,
    /** The start of the response body, for logging. */
    val body: String?,
)

internal fun interface Transport {
    /** Posts one batch. Blocking; throws [IOException] when no HTTP response arrives. */
    fun post(body: ByteArray): HttpResult
}

/**
 * Request headers, with the TS client's precedence: custom headers can replace `Content-Type` and
 * `User-Agent`, but `x-org-id`, `x-project-id` and the bearer token always win. Names are
 * compared case-insensitively.
 */
internal fun requestHeaders(
    orgId: String,
    projectId: String,
    token: String?,
    custom: Map<String, String>,
    userAgent: String,
): Map<String, String> {
    val headers = LinkedHashMap<String, String>()
    fun put(name: String, value: String) {
        headers.keys.removeAll { it.equals(name, ignoreCase = true) }
        headers[name] = value
    }
    put("Content-Type", "application/json")
    put("User-Agent", userAgent)
    for ((name, value) in custom) put(name, value)
    put("x-org-id", orgId)
    put("x-project-id", projectId)
    if (!token.isNullOrEmpty()) put("Authorization", "Bearer $token")
    return headers
}

/** [Transport] over `HttpURLConnection`, so the SDK adds no HTTP library to the app. */
internal class HttpUrlConnectionTransport(
    private val endpoint: URL,
    private val headers: Map<String, String>,
    private val connectTimeoutMillis: Int = 15_000,
    private val readTimeoutMillis: Int = 30_000,
) : Transport {

    override fun post(body: ByteArray): HttpResult {
        val connection = endpoint.openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "POST"
            connection.doOutput = true
            connection.useCaches = false
            // Following a redirect would mean sending the batch again; treat it as a misconfigured endpoint.
            connection.instanceFollowRedirects = false
            connection.connectTimeout = connectTimeoutMillis
            connection.readTimeout = readTimeoutMillis
            connection.setFixedLengthStreamingMode(body.size)
            for ((name, value) in headers) connection.setRequestProperty(name, value)

            connection.outputStream.use { it.write(body) }

            val code = connection.responseCode
            val retryAfter = connection.getHeaderField("Retry-After")?.trim()?.toLongOrNull()
            val stream = if (code >= 400) connection.errorStream else connection.inputStream
            // Reading the body to the end lets the connection be reused for the next batch.
            val text = stream?.use { readStart(it) }
            return HttpResult(code, retryAfter, text)
        } catch (e: IOException) {
            connection.disconnect()
            throw e
        }
    }

    private fun readStart(input: InputStream): String {
        val start = ByteArrayOutputStream()
        val chunk = ByteArray(4096)
        while (true) {
            val read = input.read(chunk)
            if (read < 0) break
            val room = MAX_LOGGED_BODY_BYTES - start.size()
            if (room > 0) start.write(chunk, 0, minOf(read, room))
        }
        return start.toString("UTF-8")
    }

    private companion object {
        const val MAX_LOGGED_BODY_BYTES = 2048
    }
}
