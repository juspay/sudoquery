package io.juspay.sudoquery.internal

import android.content.Context
import android.os.Build
import io.juspay.sudoquery.BuildConfig
import io.juspay.sudoquery.CanonicalEvent
import io.juspay.sudoquery.SudoQueryConfig
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.serializer
import java.io.File
import java.net.URL
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/**
 * One initialized SDK. App threads only enqueue [Command]s; a single writer coroutine handles them
 * in order and an uploader coroutine does the network IO, so a slow network never blocks writes.
 */
internal class SudoQueryClient private constructor(
    private val scope: CoroutineScope,
    private val commands: Channel<Command>,
    private val logger: Logger,
) {
    private val queuedTracks = AtomicInteger()
    private val overflowWarned = AtomicBoolean(false)
    private val user = UserMirror()

    fun track(name: String, timestampMillis: Long, properties: JsonObject) {
        // Only events are capped: control commands such as setUser must never be dropped.
        if (queuedTracks.incrementAndGet() > MAX_QUEUED_TRACKS) {
            queuedTracks.decrementAndGet()
            if (overflowWarned.compareAndSet(false, true)) {
                logger.warn("More than $MAX_QUEUED_TRACKS events are waiting to be stored; dropping new ones until they are")
            }
            return
        }
        commands.trySend(Command.Track(name, timestampMillis, properties))
    }

    fun setUser(userId: String?) {
        user.set(userId)
        commands.trySend(Command.SetUser(userId))
    }

    fun currentUser(): String? = user.get()

    fun reset() {
        user.set(null)
        commands.trySend(Command.Reset)
    }

    fun flush() {
        commands.trySend(Command.Flush)
    }

    fun onForeground() {
        commands.trySend(Command.Foreground)
    }

    fun onBackground() {
        commands.trySend(Command.Background)
    }

    /** Suspends until every command sent before this call has been handled. */
    suspend fun awaitIdle() {
        val done = CompletableDeferred<Unit>()
        commands.trySend(Command.Barrier(done))
        done.await()
    }

    /** Stops the SDK's coroutines. Only for tests: the SDK otherwise lives as long as the process. */
    fun shutdown() {
        commands.close()
        scope.cancel()
    }

    private fun handle(writer: EventWriter<*>, command: Command) {
        if (command is Command.Track && queuedTracks.decrementAndGet() < MAX_QUEUED_TRACKS / 2) {
            overflowWarned.set(false)
        }
        try {
            writer.handle(command)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            logger.error("Could not handle ${command.javaClass.simpleName}", e)
        }
    }

    /** `getUser()` must reflect `setUser()` at once, before the writer has loaded the stored user. */
    private class UserMirror {
        private val lock = Any()
        private var value: String? = null
        private var setByApp = false

        fun set(userId: String?) = synchronized(lock) {
            value = userId
            setByApp = true
        }

        fun loaded(userId: String?) = synchronized(lock) {
            if (!setByApp) value = userId
        }

        fun get(): String? = synchronized(lock) { value }
    }

    internal companion object {
        const val MAX_QUEUED_TRACKS: Int = 1_000

        fun start(context: Context, config: SudoQueryConfig, logger: Logger, clock: Clock): SudoQueryClient {
            val scope = CoroutineScope(
                SupervisorJob() + Dispatchers.IO + CoroutineExceptionHandler { _, e -> logger.error("Unexpected SDK error", e) },
            )
            val commands = Channel<Command>(Channel.UNLIMITED)
            val client = SudoQueryClient(scope, commands, logger)

            scope.launch {
                // noBackupFilesDir touches the disk, so storage is set up here, not on the app's thread.
                // It is never purged by the OS, and Auto Backup never copies it to another device.
                val root = File(context.noBackupFilesDir, "sudoquery/v1")
                val store = QueueStore(root, config.batchSize, clock, logger)
                val headers = requestHeaders(
                    orgId = config.orgId,
                    projectId = config.projectId,
                    token = config.token,
                    custom = config.headers,
                    userAgent = "sudoquery-android/${BuildConfig.SDK_VERSION} (Android ${Build.VERSION.RELEASE})",
                )
                val uploader = Uploader(store, HttpUrlConnectionTransport(URL(config.endpoint), headers), clock, logger)
                val writer = EventWriter(
                    // The only place that names the event class; see CanonicalEvent in Event.kt.
                    serializer = serializer<CanonicalEvent>(),
                    buildEvent = { CanonicalEvent(it) },
                    settings = WriterSettings(
                        orgId = config.orgId,
                        projectId = config.projectId,
                        source = config.source,
                        fixedSessionId = config.sessionId,
                        sessionTimeoutMillis = config.sessionTimeoutMillis,
                    ),
                    store = store,
                    identityStore = IdentityStore(File(root, "identity.json"), logger),
                    uploads = uploader,
                    logger = logger,
                )
                launch { uploader.run() }
                client.user.loaded(writer.start())
                for (command in commands) client.handle(writer, command)
            }

            if (config.flushIntervalMillis > 0) {
                scope.launch {
                    while (true) {
                        delay(config.flushIntervalMillis)
                        commands.trySend(Command.Tick)
                    }
                }
            }

            AppLifecycle(onForeground = client::onForeground, onBackground = client::onBackground).register(logger)
            return client
        }
    }
}
