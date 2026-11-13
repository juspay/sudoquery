package io.juspay.sudoquery.internal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class IdentityTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val timeout = 30 * 60 * 1000L
    private var ids = 0
    private val newId = { "id-${++ids}" }

    @Test
    fun `the first event starts a session`() {
        val state = IdentityState(anonId = "anon").touch(T0, timeout, null, newId)
        assertEquals("id-1", state.sessionId)
        assertEquals(T0, state.lastActivityMillis)
    }

    @Test
    fun `a session continues within the timeout and ends after it`() {
        val started = IdentityState(anonId = "anon").touch(T0, timeout, null, newId)
        val continued = started.touch(T0 + timeout, timeout, null, newId)
        assertEquals(started.sessionId, continued.sessionId)

        val renewed = continued.touch(T0 + 2 * timeout + 1, timeout, null, newId)
        assertEquals("id-2", renewed.sessionId)
    }

    @Test
    fun `a clock moving backwards keeps the session and times it from the new clock`() {
        val started = IdentityState(anonId = "anon").touch(T0, timeout, null, newId)
        val back = started.touch(T0 - 3_600_000, timeout, null, newId)
        assertEquals(started.sessionId, back.sessionId)
        assertEquals(T0 - 3_600_000, back.lastActivityMillis)
    }

    @Test
    fun `a configured session id is always used`() {
        val state = IdentityState(anonId = "anon", sessionId = "old", lastActivityMillis = 0).touch(T0, timeout, "fixed", newId)
        assertEquals("fixed", state.sessionId)
        assertEquals(0, ids)
    }

    @Test
    fun `the store round-trips and replaces the file atomically`() {
        val file = temp.root.resolve("v1/identity.json")
        val store = IdentityStore(file, RecordingLogger())
        assertNull(store.load())

        val state = IdentityState(anonId = "anon", actorId = "user", sessionId = "session", lastActivityMillis = T0)
        store.save(state)
        assertEquals(state, store.load())
        assertFalse(temp.root.resolve("v1/identity.json.tmp").exists())

        store.save(state.copy(actorId = null))
        assertEquals(state.copy(actorId = null), IdentityStore(file, RecordingLogger()).load())
    }

    @Test
    fun `an unreadable file is ignored with a warning`() {
        val file = temp.newFile("identity.json").apply { writeText("{not json") }
        val logger = RecordingLogger()
        assertNull(IdentityStore(file, logger).load())
        assertEquals(1, logger.warnings.size)
    }
}
