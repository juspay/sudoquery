package io.juspay.sudoquery.internal

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.math.BigDecimal

class JsonValuesTest {
    private val logger = RecordingLogger()

    private enum class Plan { PRO }

    @Test
    fun `converts nested maps, lists and arrays in order`() {
        val json = JsonValues.toJsonObject(
            linkedMapOf(
                "string" to "a",
                "int" to 1,
                "long" to 2L,
                "double" to 1.5,
                "decimal" to BigDecimal("10.25"),
                "bool" to true,
                "null" to null,
                "char" to 'x',
                "enum" to Plan.PRO,
                "list" to listOf(1, "two"),
                "array" to arrayOf("a", "b"),
                "ints" to intArrayOf(1, 2),
                "nested" to mapOf("inner" to mapOf("deep" to 1)),
            ),
            logger,
        )
        assertEquals(
            """{"string":"a","int":1,"long":2,"double":1.5,"decimal":10.25,"bool":true,"null":null,"char":"x",""" +
                """"enum":"PRO","list":[1,"two"],"array":["a","b"],"ints":[1,2],"nested":{"inner":{"deep":1}}}""",
            json.toString(),
        )
    }

    @Test
    fun `non-finite numbers become null with a warning`() {
        val json = JsonValues.toJsonObject(
            mapOf("nan" to Double.NaN, "inf" to Float.POSITIVE_INFINITY, "list" to doubleArrayOf(1.0, Double.NEGATIVE_INFINITY)),
            logger,
        )
        assertEquals("""{"nan":null,"inf":null,"list":[1.0,null]}""", json.toString())
        assertEquals(3, logger.warnings.size)
    }

    @Test
    fun `converts org json values`() {
        val source = JSONObject()
            .put("a", 1)
            .put("none", JSONObject.NULL)
            .put("list", JSONArray().put("x").put(JSONObject().put("y", false)))
        assertEquals("""{"a":1,"none":null,"list":["x",{"y":false}]}""", JsonValues.toJsonObject(source, logger).toString())
    }

    @Test
    fun `a map containing itself stops at the depth limit`() {
        val loop = HashMap<String, Any?>()
        loop["self"] = loop
        val json = JsonValues.toJsonObject(loop, logger)
        var depth = 0
        var current: Any = json
        while (current is JsonObject) {
            current = current.getValue("self")
            depth++
        }
        assertEquals(JsonNull, current)
        assertEquals(33, depth)
        assertTrue(logger.warnings.single().contains("nested deeper"))
    }

    @Test
    fun `the result is a copy`() {
        val properties = mutableMapOf<String, Any?>("items" to mutableListOf("a"))
        val json = JsonValues.toJsonObject(properties, logger)
        @Suppress("UNCHECKED_CAST")
        (properties["items"] as MutableList<String>).add("b")
        properties["late"] = 1
        assertEquals("""{"items":["a"]}""", json.toString())
    }

    @Test
    fun `toPlain turns JSON back into maps, lists and primitives`() {
        val element = JsonObject(
            mapOf(
                "s" to JsonPrimitive("x"),
                "n" to JsonPrimitive(3),
                "d" to JsonPrimitive(2.5),
                "b" to JsonPrimitive(true),
                "null" to JsonNull,
                "obj" to JsonObject(mapOf("k" to JsonArray(listOf(JsonPrimitive(1))))),
            ),
        )
        assertEquals(
            mapOf("s" to "x", "n" to 3L, "d" to 2.5, "b" to true, "null" to null, "obj" to mapOf("k" to listOf(1L))),
            JsonValues.toPlain(element),
        )
        assertEquals("x", JsonValues.toPlain(JsonValues.toJsonObject(mapOf("s" to "x"), logger).jsonObject.getValue("s")))
    }
}
