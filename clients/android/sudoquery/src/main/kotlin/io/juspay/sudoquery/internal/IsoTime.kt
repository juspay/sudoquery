package io.juspay.sudoquery.internal

private const val MILLIS_PER_DAY = 86_400_000L

/**
 * Formats epoch milliseconds as RFC 3339 in UTC with milliseconds, e.g. `2026-10-14T09:30:12.345Z`,
 * the same as `Date.toISOString()` in the TS client.
 *
 * Plain arithmetic: `java.time` needs API 26, and `SimpleDateFormat` is slow and not thread-safe.
 * The date conversion is Howard Hinnant's `civil_from_days`.
 */
internal fun formatIsoUtcMillis(epochMillis: Long): String {
    val days = epochMillis.floorDiv(MILLIS_PER_DAY)
    val millisOfDay = epochMillis.mod(MILLIS_PER_DAY)

    val z = days + 719_468
    val era = z.floorDiv(146_097L)
    val dayOfEra = z - era * 146_097
    val yearOfEra = (dayOfEra - dayOfEra / 1_460 + dayOfEra / 36_524 - dayOfEra / 146_096) / 365
    val dayOfYear = dayOfEra - (365 * yearOfEra + yearOfEra / 4 - yearOfEra / 100)
    val shiftedMonth = (5 * dayOfYear + 2) / 153
    val day = dayOfYear - (153 * shiftedMonth + 2) / 5 + 1
    val month = if (shiftedMonth < 10) shiftedMonth + 3 else shiftedMonth - 9
    val year = yearOfEra + era * 400 + if (month <= 2) 1 else 0

    return buildString(24) {
        appendPadded(year, 4)
        append('-')
        appendPadded(month, 2)
        append('-')
        appendPadded(day, 2)
        append('T')
        appendPadded(millisOfDay / 3_600_000, 2)
        append(':')
        appendPadded(millisOfDay / 60_000 % 60, 2)
        append(':')
        appendPadded(millisOfDay / 1_000 % 60, 2)
        append('.')
        appendPadded(millisOfDay % 1_000, 3)
        append('Z')
    }
}

private fun StringBuilder.appendPadded(value: Long, width: Int) {
    val digits = value.toString()
    repeat(width - digits.length) { append('0') }
    append(digits)
}
