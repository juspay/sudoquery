package io.juspay.sudoquery.internal

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull
import org.json.JSONArray
import org.json.JSONObject

/**
 * Converts values passed to `track()` and `setSuperProperty()` into JSON. The result is a copy,
 * so a caller changing its map afterwards can't alter an event that is already queued.
 */
internal object JsonValues {
    /** Deeper values become null; this also stops a map that contains itself. */
    private const val MAX_DEPTH = 32

    fun toJsonObject(map: Map<*, *>, logger: Logger): JsonObject = convertMap(map, 0, logger)

    fun toJsonObject(json: JSONObject, logger: Logger): JsonObject = convertJSONObject(json, 0, logger)

    fun toJsonElement(value: Any?, logger: Logger): JsonElement = convert(value, 0, logger)

    /** The reverse, for handing super properties back to the app: objects become maps, arrays lists. */
    fun toPlain(element: JsonElement): Any? = when (element) {
        JsonNull -> null
        is JsonPrimitive -> when {
            element.isString -> element.content
            else -> element.booleanOrNull ?: element.longOrNull ?: element.doubleOrNull ?: element.content
        }
        is JsonObject -> element.mapValues { toPlain(it.value) }
        is JsonArray -> element.map { toPlain(it) }
    }

    private fun convert(value: Any?, depth: Int, logger: Logger): JsonElement {
        if (value == null || value === JSONObject.NULL) return JsonNull
        if (depth > MAX_DEPTH) {
            logger.warn("A property nested deeper than $MAX_DEPTH levels was replaced with null")
            return JsonNull
        }
        return when (value) {
            is JsonElement -> value
            is String -> JsonPrimitive(value)
            is Boolean -> JsonPrimitive(value)
            is Double -> finite(value.isFinite(), value, logger)
            is Float -> finite(value.isFinite(), value, logger)
            is Number -> JsonPrimitive(value)
            is Char -> JsonPrimitive(value.toString())
            is Enum<*> -> JsonPrimitive(value.name)
            is Map<*, *> -> convertMap(value, depth, logger)
            is JSONObject -> convertJSONObject(value, depth, logger)
            is JSONArray -> JsonArray(List(value.length()) { convert(value.opt(it), depth + 1, logger) })
            is Iterable<*> -> JsonArray(value.map { convert(it, depth + 1, logger) })
            is Array<*> -> JsonArray(value.map { convert(it, depth + 1, logger) })
            is BooleanArray -> JsonArray(value.map { JsonPrimitive(it) })
            is ByteArray -> JsonArray(value.map { JsonPrimitive(it) })
            is ShortArray -> JsonArray(value.map { JsonPrimitive(it) })
            is IntArray -> JsonArray(value.map { JsonPrimitive(it) })
            is LongArray -> JsonArray(value.map { JsonPrimitive(it) })
            is FloatArray -> JsonArray(value.map { finite(it.isFinite(), it, logger) })
            is DoubleArray -> JsonArray(value.map { finite(it.isFinite(), it, logger) })
            is CharArray -> JsonArray(value.map { JsonPrimitive(it.toString()) })
            else -> JsonPrimitive(value.toString())
        }
    }

    private fun convertMap(map: Map<*, *>, depth: Int, logger: Logger): JsonObject {
        val content = LinkedHashMap<String, JsonElement>(map.size)
        for ((key, value) in map) content[key.toString()] = convert(value, depth + 1, logger)
        return JsonObject(content)
    }

    private fun convertJSONObject(json: JSONObject, depth: Int, logger: Logger): JsonObject {
        val content = LinkedHashMap<String, JsonElement>(json.length())
        for (key in json.keys()) content[key] = convert(json.opt(key), depth + 1, logger)
        return JsonObject(content)
    }

    /** JSON has no NaN or Infinity; sending one would make the collector reject the whole batch. */
    private fun finite(isFinite: Boolean, value: Number, logger: Logger): JsonElement {
        if (isFinite) return JsonPrimitive(value)
        logger.warn("A property value of $value can't be sent as JSON and was replaced with null")
        return JsonNull
    }
}
