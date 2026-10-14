package io.juspay.sudoquery

import android.content.Context
import io.juspay.sudoquery.internal.AndroidClock
import io.juspay.sudoquery.internal.AndroidLogger
import io.juspay.sudoquery.internal.Clock
import io.juspay.sudoquery.internal.JsonValues
import io.juspay.sudoquery.internal.Logger
import io.juspay.sudoquery.internal.SudoQueryClient
import io.juspay.sudoquery.internal.SuperProperties
import io.juspay.sudoquery.internal.isMainProcess
import kotlinx.serialization.json.JsonObject
import org.json.JSONObject
import java.util.concurrent.atomic.AtomicBoolean

/**
 * The SudoQuery Android SDK. Call [init] once, typically in `Application.onCreate`, then [track]
 * events from anywhere.
 *
 * Every method is thread-safe, returns quickly and never throws: events are written to disk and
 * sent to the collector on background threads. Events survive app restarts and are retried until
 * the collector accepts them (at-least-once delivery).
 */
public object SudoQuery {
    private val lock = Any()
    private val superProperties = SuperProperties()
    private val warnedNotInitialized = AtomicBoolean(false)

    @Volatile
    private var client: SudoQueryClient? = null

    @Volatile
    private var logger: Logger = AndroidLogger(debugEnabled = false)

    // Seams for tests.
    @Volatile
    internal var processCheck: (Context) -> Boolean = ::isMainProcess

    @Volatile
    internal var clock: Clock = AndroidClock

    /**
     * Starts the SDK. Only the first call has an effect; later calls are ignored, as in the TS
     * client. Does nothing outside the app's main process.
     */
    @JvmStatic
    public fun init(context: Context, config: SudoQueryConfig) {
        guarded("init") {
            synchronized(lock) {
                if (client != null) {
                    logger.debug("init called again; keeping the first configuration")
                    return
                }
                val app = context.applicationContext ?: context
                val configLogger = AndroidLogger(config.debugLogging)
                if (!processCheck(app)) {
                    configLogger.warn("Not starting: SudoQuery only runs in the app's main process")
                    return
                }
                warnIfNotSlug("orgId", config.orgId, configLogger)
                warnIfNotSlug("projectId", config.projectId, configLogger)
                logger = configLogger
                client = SudoQueryClient.start(app, config, configLogger, clock)
            }
        }
    }

    /** True once [init] has started the SDK. */
    @JvmStatic
    public val isInitialized: Boolean
        get() = client != null

    /**
     * Records an event. [properties] are copied and sent as a JSON object, merged over the super
     * properties (keys given here win). Nested maps, lists and arrays are supported; NaN and
     * infinite numbers become null. Events tracked before [init] are dropped with a warning.
     */
    @JvmStatic
    @JvmOverloads
    public fun track(name: String, properties: Map<String, Any?> = emptyMap()) {
        val timestamp = clock.nowMillis()
        guarded("track") {
            val active = clientOrWarn("track") ?: return
            active.track(name, timestamp, merged(JsonValues.toJsonObject(properties, logger)))
        }
    }

    /** Records an event with properties from an `org.json` object. See [track]. */
    @JvmStatic
    public fun track(name: String, properties: JSONObject) {
        val timestamp = clock.nowMillis()
        guarded("track") {
            val active = clientOrWarn("track") ?: return
            active.track(name, timestamp, merged(JsonValues.toJsonObject(properties, logger)))
        }
    }

    /** Sets the user id sent as `actor_id` with every later event. It is kept across app restarts. */
    @JvmStatic
    public fun setUser(userId: String) {
        guarded("setUser") { clientOrWarn("setUser")?.setUser(userId) }
    }

    /** Stops sending a user id. The anonymous id and session are kept; see [reset] for a full logout. */
    @JvmStatic
    public fun removeUser() {
        guarded("removeUser") { clientOrWarn("removeUser")?.setUser(null) }
    }

    /** The current user id, or null. */
    @JvmStatic
    public fun getUser(): String? = client?.currentUser()

    /** Adds a property to every later event. Kept in memory only, and usable before [init]. */
    @JvmStatic
    public fun setSuperProperty(key: String, value: Any?) {
        guarded("setSuperProperty") { superProperties.set(key, JsonValues.toJsonElement(value, logger)) }
    }

    /** A copy of the super properties; JSON objects come back as maps and arrays as lists. */
    @JvmStatic
    public fun getSuperProperties(): Map<String, Any?> =
        superProperties.snapshot().mapValues { JsonValues.toPlain(it.value) }

    @JvmStatic
    public fun clearSuperProperties() {
        superProperties.clear()
    }

    /**
     * Commits the events tracked so far into a batch and starts sending it. Returns at once;
     * if uploads are failing, the batch waits for the next retry.
     */
    @JvmStatic
    public fun flush() {
        guarded("flush") { clientOrWarn("flush")?.flush() }
    }

    /**
     * For logout: later events get a new anonymous id and session, and no user id. Super
     * properties are cleared. Events already tracked are still sent unchanged.
     */
    @JvmStatic
    public fun reset() {
        guarded("reset") {
            superProperties.clear()
            clientOrWarn("reset")?.reset()
        }
    }

    internal val clientForTests: SudoQueryClient?
        get() = client

    /** Stops the SDK and forgets its state, so a test can call [init] again. */
    internal fun resetForTests() {
        synchronized(lock) {
            client?.shutdown()
            client = null
            superProperties.clear()
            warnedNotInitialized.set(false)
            processCheck = ::isMainProcess
            clock = AndroidClock
        }
    }

    private fun merged(properties: JsonObject): JsonObject = superProperties.mergeUnder(properties)

    private fun clientOrWarn(operation: String): SudoQueryClient? {
        val current = client
        if (current == null && warnedNotInitialized.compareAndSet(false, true)) {
            logger.warn("SudoQuery.$operation was called before SudoQuery.init; such calls are ignored")
        }
        return current
    }

    private inline fun guarded(operation: String, block: () -> Unit) {
        try {
            block()
        } catch (e: Exception) {
            logger.error("SudoQuery.$operation failed", e)
        }
    }

    private val SLUG = Regex("^[a-z][a-z0-9-]{4,28}[a-z0-9]$")

    /**
     * Org and project ids are server-generated slugs (e.g. "acme-store-k3x9qa"). Warn, never throw,
     * when one deviates, so the misconfiguration is visible without breaking the app.
     */
    private fun warnIfNotSlug(field: String, value: String, logger: Logger) {
        if (SLUG.matches(value)) return
        logger.warn(
            "config option \"$field\" value \"$value\" does not match the expected server-generated slug shape " +
                "/^[a-z][a-z0-9-]{4,28}[a-z0-9]$/ (e.g. \"acme-store-k3x9qa\"); the collector may reject events carrying it.",
        )
    }
}
