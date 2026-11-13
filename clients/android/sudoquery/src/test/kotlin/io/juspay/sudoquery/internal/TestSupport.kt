package io.juspay.sudoquery.internal

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import java.io.File
import java.io.IOException

/** 2025-10-09T08:53:20Z, a fixed "now" for tests. */
internal const val T0: Long = 1_760_000_000_000L

internal class FakeClock(var now: Long = T0, var elapsed: Long = 0L) : Clock {
    override fun nowMillis(): Long = now

    override fun elapsedMillis(): Long = elapsed
}

internal class RecordingLogger : Logger {
    val warnings: MutableList<String> = mutableListOf()
    val errors: MutableList<String> = mutableListOf()

    override fun debug(message: String) = Unit

    @Synchronized
    override fun warn(message: String, error: Throwable?) {
        warnings += message
    }

    @Synchronized
    override fun error(message: String, error: Throwable?) {
        errors += message
    }
}

internal class FakeUploads : UploadControl {
    var requests = 0
    var resets = 0
    override var isBackingOff = false

    override fun requestUpload() {
        requests++
    }

    override fun resetBackoff() {
        resets++
        isBackingOff = false
    }
}

/** Replies with the queued outcomes in order, then with 200. Records every body it was given. */
internal class FakeTransport : Transport {
    val bodies: MutableList<String> = mutableListOf()
    private val outcomes = ArrayDeque<() -> HttpResult>()

    fun reply(code: Int, body: String = "{}", retryAfterSeconds: Long? = null) {
        outcomes.addLast { HttpResult(code, retryAfterSeconds, body) }
    }

    fun failWith(error: IOException) {
        outcomes.addLast { throw error }
    }

    override fun post(body: ByteArray): HttpResult {
        bodies += body.decodeToString()
        val next = outcomes.removeFirstOrNull() ?: return HttpResult(200, null, """{"filtered":0,"collected":1,"total":1}""")
        return next()
    }

    /** The events of the [index]th request. */
    fun events(index: Int): List<JsonObject> =
        Json.parseToJsonElement(bodies[index]).jsonObject.getValue("events").jsonArray.map { it.jsonObject }
}

internal fun File.ndjsonLines(): List<JsonObject> =
    readLines().filter { it.isNotBlank() }.map { Json.parseToJsonElement(it).jsonObject }

internal fun JsonArray.objects(): List<JsonObject> = map { it.jsonObject }
