package io.juspay.sudoquery.internal

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/** Properties added to every event. In memory only, as in the TS client. */
internal class SuperProperties {
    private val lock = Any()

    @Volatile
    private var values: Map<String, JsonElement> = emptyMap()

    fun set(key: String, value: JsonElement) {
        synchronized(lock) { values = LinkedHashMap(values).apply { put(key, value) } }
    }

    fun clear() {
        synchronized(lock) { values = emptyMap() }
    }

    fun snapshot(): Map<String, JsonElement> = values

    /** The event's own properties win, like `{...superProperties, ...properties}` in the TS client. */
    fun mergeUnder(properties: JsonObject): JsonObject {
        val current = values
        if (current.isEmpty()) return properties
        return JsonObject(LinkedHashMap(current).apply { putAll(properties) })
    }
}
