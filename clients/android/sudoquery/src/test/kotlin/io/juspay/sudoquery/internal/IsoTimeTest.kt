package io.juspay.sudoquery.internal

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import kotlin.random.Random

class IsoTimeTest {
    private val reference = DateTimeFormatter.ofPattern("uuuu-MM-dd'T'HH:mm:ss.SSS'Z'").withZone(ZoneOffset.UTC)

    private fun expected(millis: Long) = reference.format(Instant.ofEpochMilli(millis))

    @Test
    fun `formats like Date toISOString`() {
        assertEquals("2026-10-14T09:30:12.345Z", formatIsoUtcMillis(Instant.parse("2026-10-14T09:30:12.345Z").toEpochMilli()))
        assertEquals("1970-01-01T00:00:00.000Z", formatIsoUtcMillis(0))
        assertEquals("1969-12-31T23:59:59.999Z", formatIsoUtcMillis(-1))
    }

    @Test
    fun `handles leap days and year ends`() {
        listOf(
            "2000-02-29T12:00:00.001Z",
            "2024-02-29T23:59:59.999Z",
            "2100-03-01T00:00:00.000Z",
            "2026-12-31T23:59:59.999Z",
            "2027-01-01T00:00:00.000Z",
        ).forEach { iso -> assertEquals(iso, formatIsoUtcMillis(Instant.parse(iso).toEpochMilli())) }
    }

    @Test
    fun `matches java time across two centuries`() {
        val random = Random(42)
        val end = Instant.parse("2170-01-01T00:00:00Z").toEpochMilli()
        repeat(20_000) {
            val millis = random.nextLong(0, end)
            assertEquals(expected(millis), formatIsoUtcMillis(millis))
        }
    }
}
