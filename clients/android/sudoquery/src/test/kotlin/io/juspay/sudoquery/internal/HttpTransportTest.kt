package io.juspay.sudoquery.internal

import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.Headers.Companion.headersOf
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.io.IOException
import java.util.concurrent.TimeUnit

class HttpTransportTest {
    private val server = MockWebServer()

    @Before
    fun start() = server.start()

    @After
    fun stop() = server.close()

    private fun transport(headers: Map<String, String> = mapOf("Content-Type" to "application/json")) =
        HttpUrlConnectionTransport(server.url("/v1/events/batch").toUrl(), headers)

    @Test
    fun `posts the body with the given headers`() {
        server.enqueue(MockResponse(code = 200, body = """{"filtered":0,"collected":1,"total":1}"""))
        val headers = requestHeaders("acme-k3x9qa", "android-demo1", "secret", mapOf("X-Api-Key" to "k"), "sudoquery-android/0.1.0")

        val result = transport(headers).post("""{"events":[]}""".toByteArray())

        assertEquals(200, result.code)
        assertEquals("""{"filtered":0,"collected":1,"total":1}""", result.body)
        val request = server.takeRequest(5, TimeUnit.SECONDS)!!
        assertEquals("POST", request.method)
        assertEquals("/v1/events/batch", request.url.encodedPath)
        assertEquals("""{"events":[]}""", request.body!!.utf8())
        assertEquals("application/json", request.headers["Content-Type"])
        assertEquals("acme-k3x9qa", request.headers["x-org-id"])
        assertEquals("android-demo1", request.headers["x-project-id"])
        assertEquals("Bearer secret", request.headers["Authorization"])
        assertEquals("k", request.headers["X-Api-Key"])
        assertEquals("sudoquery-android/0.1.0", request.headers["User-Agent"])
    }

    @Test
    fun `returns error responses with their body and Retry-After`() {
        server.enqueue(MockResponse(code = 400, body = """{"error":"bad"}"""))
        server.enqueue(MockResponse(code = 429, headers = headersOf("Retry-After", "30")))

        val rejected = transport().post(ByteArray(0))
        assertEquals(400, rejected.code)
        assertEquals("""{"error":"bad"}""", rejected.body)
        assertNull(rejected.retryAfterSeconds)

        assertEquals(30L, transport().post(ByteArray(0)).retryAfterSeconds)
    }

    @Test
    fun `does not follow redirects`() {
        server.enqueue(MockResponse(code = 307, headers = headersOf("Location", server.url("/elsewhere").toString())))
        assertEquals(307, transport().post(ByteArray(1)).code)
        assertEquals(1, server.requestCount)
    }

    @Test
    fun `throws IOException when the collector is unreachable`() {
        val unreachable = transport()
        server.close()
        try {
            unreachable.post(ByteArray(1))
            fail("expected an IOException")
        } catch (expected: IOException) {
        }
    }

    @Test
    fun `custom headers can't replace the ids or the token`() {
        val headers = requestHeaders(
            orgId = "acme-k3x9qa",
            projectId = "android-demo1",
            token = "secret",
            custom = mapOf(
                "content-type" to "application/x-ndjson",
                "X-Org-Id" to "other-org",
                "authorization" to "Basic abc",
                "user-agent" to "custom",
            ),
            userAgent = "sudoquery-android/0.1.0",
        )
        assertEquals(
            mapOf(
                "content-type" to "application/x-ndjson",
                "user-agent" to "custom",
                "x-org-id" to "acme-k3x9qa",
                "x-project-id" to "android-demo1",
                "Authorization" to "Bearer secret",
            ),
            headers,
        )
    }

    @Test
    fun `an empty token is not sent`() {
        val headers = requestHeaders("acme-k3x9qa", "android-demo1", "", emptyMap(), "ua")
        assertEquals(listOf("Content-Type", "User-Agent", "x-org-id", "x-project-id"), headers.keys.toList())
    }
}
