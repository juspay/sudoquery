package io.juspay.sudoquery.internal

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.File
import java.io.FileOutputStream
import java.io.IOException

/** Who events belong to. Persisted so the anonymous id, user and session survive app restarts. */
@Serializable
internal data class IdentityState(
    @SerialName("anon_id") val anonId: String,
    @SerialName("actor_id") val actorId: String? = null,
    @SerialName("session_id") val sessionId: String? = null,
    @SerialName("last_activity_ms") val lastActivityMillis: Long = 0,
)

/**
 * Applies the session rules for an event at [nowMillis]: a session ends after [timeoutMillis]
 * without events. If the clock moved backwards the session continues, timed from the new clock.
 * A [fixedSessionId] from the config is used as is.
 */
internal fun IdentityState.touch(
    nowMillis: Long,
    timeoutMillis: Long,
    fixedSessionId: String?,
    newId: () -> String,
): IdentityState {
    val session = when {
        fixedSessionId != null -> fixedSessionId
        sessionId == null || nowMillis - lastActivityMillis > timeoutMillis -> newId()
        else -> sessionId
    }
    return copy(sessionId = session, lastActivityMillis = nowMillis)
}

/** Keeps [IdentityState] in one small JSON file, replaced atomically so a crash can't leave it half-written. */
internal class IdentityStore(private val file: File, private val logger: Logger) {

    fun load(): IdentityState? {
        if (!file.exists()) return null
        return try {
            json.decodeFromString(IdentityState.serializer(), file.readText(Charsets.UTF_8))
        } catch (e: Exception) {
            logger.warn("Ignoring unreadable ${file.name}; starting with a new anonymous id", e)
            null
        }
    }

    fun save(state: IdentityState) {
        val temp = File(file.path + ".tmp")
        try {
            file.parentFile?.mkdirs()
            FileOutputStream(temp).use { out ->
                out.write(json.encodeToString(IdentityState.serializer(), state).toByteArray(Charsets.UTF_8))
                out.fd.sync()
            }
            if (!temp.renameTo(file)) throw IOException("could not rename ${temp.name} to ${file.name}")
        } catch (e: IOException) {
            logger.warn("Could not save the SDK identity", e)
        }
    }

    private companion object {
        val json = Json { ignoreUnknownKeys = true }
    }
}
