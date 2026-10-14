package io.juspay.sudoquery

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.util.concurrent.TimeUnit

@RunWith(RobolectricTestRunner::class)
class SudoQueryFacadeTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val server = MockWebServer()

    @Before
    fun setUp() {
        server.start()
        SudoQuery.resetForTests()
    }

    @After
    fun tearDown() {
        SudoQuery.resetForTests()
        server.close()
        File(context.noBackupFilesDir, "sudoquery").deleteRecursively()
    }

    private fun config(batchSize: Int = 2, endpoint: String = server.url("/v1/events/batch").toString()) =
        SudoQueryConfig.Builder("acme-k3x9qa", "android-demo1", endpoint)
            .batchSize(batchSize)
            .flushIntervalMillis(0)
            .build()

    @Test
    fun `calls before init are ignored without throwing`() {
        SudoQuery.track("early", mapOf("k" to 1))
        SudoQuery.setUser("u1")
        SudoQuery.removeUser()
        SudoQuery.flush()
        SudoQuery.reset()
        assertFalse(SudoQuery.isInitialized)
        assertNull(SudoQuery.getUser())
    }

    @Test
    fun `super properties work before init and are copies`() {
        SudoQuery.setSuperProperty("plan", "pro")
        SudoQuery.setSuperProperty("tags", listOf("a", "b"))
        assertEquals(mapOf("plan" to "pro", "tags" to listOf("a", "b")), SudoQuery.getSuperProperties())
        SudoQuery.clearSuperProperties()
        assertEquals(emptyMap<String, Any?>(), SudoQuery.getSuperProperties())
    }

    @Test
    fun `the first init wins`() {
        SudoQuery.init(context, config())
        val first = SudoQuery.clientForTests
        SudoQuery.init(context, config(endpoint = "https://other.example.com/v1/events/batch"))
        assertTrue(SudoQuery.isInitialized)
        assertTrue(first === SudoQuery.clientForTests)
    }

    @Test
    fun `init does nothing outside the main process`() {
        SudoQuery.processCheck = { false }
        SudoQuery.init(context, config())
        assertFalse(SudoQuery.isInitialized)
    }

    @Test
    fun `invalid config is rejected at build time`() {
        assertThrows { SudoQueryConfig.Builder("", "android-demo1", "https://c.example.com/v1/events/batch").build() }
        assertThrows { SudoQueryConfig.Builder("acme-k3x9qa", "android-demo1", "collector/v1/events/batch").build() }
        assertThrows { SudoQueryConfig.Builder("acme-k3x9qa", "android-demo1", "ftp://c.example.com").build() }
        assertThrows { SudoQueryConfig.Builder("acme-k3x9qa", "android-demo1", "https://c.example.com").batchSize(0).build() }
    }

    @Test
    fun `tracked events reach the collector with identity and super properties`() {
        server.enqueue(MockResponse(code = 200, body = """{"filtered":0,"collected":2,"total":2}"""))
        SudoQuery.init(context, config(batchSize = 2))
        SudoQuery.setSuperProperty("app_version", "1.0.0")
        SudoQuery.setUser("u1")
        assertEquals("u1", SudoQuery.getUser())

        SudoQuery.track("checkout_started", mapOf("cart_value" to 499.5, "app_version" to "override"))
        SudoQuery.track("checkout_completed")

        val request = checkNotNull(server.takeRequest(10, TimeUnit.SECONDS)) { "no request reached the server" }
        assertEquals("acme-k3x9qa", request.headers["x-org-id"])
        assertEquals("android-demo1", request.headers["x-project-id"])
        assertTrue(request.headers["User-Agent"]!!.startsWith("sudoquery-android/"))

        val body = Json.parseToJsonElement(request.body!!.utf8()).jsonObject
        val events = body.getValue("events").jsonArray.map { it.jsonObject }
        assertEquals(listOf("checkout_started", "checkout_completed"), events.map { it.text("name") })
        assertEquals(listOf("u1", "u1"), events.map { it.text("actor_id") })
        assertEquals(events[0].text("anon_id"), events[1].text("anon_id"))
        assertEquals(events[0].text("session_id"), events[1].text("session_id"))
        assertEquals("""{"app_version":"override","cart_value":499.5}""", events[0].getValue("properties").toString())
        assertEquals("""{"app_version":"1.0.0"}""", events[1].getValue("properties").toString())
        assertTrue(body.getValue("system_properties").jsonObject.containsKey("timezone"))
    }

    @Test
    fun `reset clears the user and super properties`() {
        SudoQuery.init(context, config())
        SudoQuery.setUser("u1")
        SudoQuery.setSuperProperty("plan", "pro")
        SudoQuery.reset()
        assertNull(SudoQuery.getUser())
        assertEquals(emptyMap<String, Any?>(), SudoQuery.getSuperProperties())
        runBlocking { withTimeout(10_000) { SudoQuery.clientForTests!!.awaitIdle() } }
    }

    private fun JsonObject.text(key: String): String? = getValue(key).let { if (it is JsonNull) null else it.jsonPrimitive.content }

    private fun assertThrows(block: () -> Unit) {
        try {
            block()
        } catch (expected: IllegalArgumentException) {
            return
        }
        throw AssertionError("expected IllegalArgumentException")
    }
}
