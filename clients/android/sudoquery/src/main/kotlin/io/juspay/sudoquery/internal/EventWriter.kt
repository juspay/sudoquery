package io.juspay.sudoquery.internal

import io.juspay.sudoquery.EventInput
import io.juspay.sudoquery.WireJson
import kotlinx.coroutines.CompletableDeferred
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonObject
import java.io.IOException
import java.util.UUID

/** Work for the event writer, handled strictly in the order the app made the calls. */
internal sealed interface Command {
    class Track(val name: String, val timestampMillis: Long, val properties: JsonObject) : Command

    class SetUser(val userId: String?) : Command

    object Reset : Command

    object Flush : Command

    object Tick : Command

    object Background : Command

    object Foreground : Command

    /** Completes once every earlier command has been handled. */
    class Barrier(val done: CompletableDeferred<Unit>) : Command
}

internal class WriterSettings(
    val orgId: String,
    val projectId: String,
    val source: String?,
    val fixedSessionId: String?,
    val sessionTimeoutMillis: Long,
)

/**
 * Turns commands into stored events. It runs on a single coroutine, so `setUser` → `track` →
 * `removeUser` keeps its meaning, and identity is resolved here (after it was loaded from disk)
 * rather than on the app's thread.
 *
 * Generic over the event class and its serializer; the SDK wires it with `CanonicalEvent`. Each
 * event is serialized exactly once, so a resend carries the same `id` and timestamp.
 */
internal class EventWriter<E>(
    private val serializer: KSerializer<E>,
    private val buildEvent: (EventInput) -> E,
    private val settings: WriterSettings,
    private val store: QueueStore,
    private val identityStore: IdentityStore,
    private val uploads: UploadControl,
    private val logger: Logger,
    private val newId: () -> String = { UUID.randomUUID().toString() },
) {
    private lateinit var identity: IdentityState
    private var identityDirty = false

    /** Loads the identity and recovers batches left by the previous run. Returns the stored user id. */
    fun start(): String? {
        identity = identityStore.load() ?: IdentityState(anonId = newId()).also { identityStore.save(it) }
        try {
            store.recover()
            store.enforceLimits()
        } catch (e: Exception) {
            logger.error("Could not recover the event queue", e)
        }
        uploads.requestUpload()
        return identity.actorId
    }

    fun handle(command: Command) {
        when (command) {
            is Command.Track -> track(command)
            is Command.SetUser -> updateIdentity(identity.copy(actorId = command.userId))
            Command.Reset -> updateIdentity(IdentityState(anonId = newId()))
            Command.Flush -> commitAndUpload()
            Command.Tick -> {
                // While uploads back off (e.g. offline), let the open batch fill up instead of
                // committing many small ones.
                if (!uploads.isBackingOff) commitAndUpload()
                saveIdentityIfDirty()
            }
            Command.Background -> {
                commitAndUpload()
                saveIdentityIfDirty()
            }
            Command.Foreground -> {
                uploads.resetBackoff()
                uploads.requestUpload()
            }
            is Command.Barrier -> command.done.complete(Unit)
        }
    }

    private fun track(command: Command.Track) {
        val previousSession = identity.sessionId
        identity = identity.touch(command.timestampMillis, settings.sessionTimeoutMillis, settings.fixedSessionId, newId)
        if (identity.sessionId != previousSession) updateIdentity(identity) else identityDirty = true

        val input = EventInput(
            id = newId(),
            name = command.name,
            orgId = settings.orgId,
            projectId = settings.projectId,
            sessionId = identity.sessionId,
            anonId = identity.anonId,
            actorId = identity.actorId,
            source = settings.source,
            occurredAt = formatIsoUtcMillis(command.timestampMillis),
            properties = command.properties,
        )
        val line = WireJson.encodeToString(serializer, buildEvent(input)).toByteArray(Charsets.UTF_8)
        if (line.size > MAX_EVENT_BYTES) {
            logger.warn("Dropped event \"${command.name}\": it is ${line.size} bytes, over the $MAX_EVENT_BYTES-byte limit")
            return
        }
        try {
            if (store.append(line)) afterCommit()
        } catch (e: IOException) {
            logger.warn("Could not store event \"${command.name}\"; it was dropped", e)
        }
    }

    private fun commitAndUpload() {
        try {
            if (store.commit()) store.enforceLimits()
        } catch (e: IOException) {
            logger.warn("Could not commit the open batch", e)
        }
        uploads.requestUpload()
    }

    private fun afterCommit() {
        store.enforceLimits()
        uploads.requestUpload()
    }

    private fun updateIdentity(state: IdentityState) {
        identity = state
        identityStore.save(state)
        identityDirty = false
    }

    private fun saveIdentityIfDirty() {
        if (identityDirty) updateIdentity(identity)
    }

    internal companion object {
        /** Larger events are dropped: they would crowd out whole batches. */
        const val MAX_EVENT_BYTES: Int = 64 * 1024
    }
}
