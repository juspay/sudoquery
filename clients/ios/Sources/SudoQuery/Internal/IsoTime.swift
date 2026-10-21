private let millisPerDay: Int64 = 86_400_000

/// Formats epoch milliseconds as RFC 3339 in UTC with milliseconds, e.g. `2026-10-14T09:30:12.345Z`,
/// the same as `Date.toISOString()` in the TS client.
///
/// Plain arithmetic, as in the Android SDK: no formatter to create or share between threads, and
/// no rounding through `Double`. The date conversion is Howard Hinnant's `civil_from_days`.
func formatIsoUtcMillis(_ epochMillis: Int64) -> String {
    let days = floorDiv(epochMillis, millisPerDay)
    let millisOfDay = floorMod(epochMillis, millisPerDay)

    let z = days + 719_468
    let era = floorDiv(z, 146_097)
    let dayOfEra = z - era * 146_097
    let yearOfEra = (dayOfEra - dayOfEra / 1_460 + dayOfEra / 36_524 - dayOfEra / 146_096) / 365
    let dayOfYear = dayOfEra - (365 * yearOfEra + yearOfEra / 4 - yearOfEra / 100)
    let shiftedMonth = (5 * dayOfYear + 2) / 153
    let day = dayOfYear - (153 * shiftedMonth + 2) / 5 + 1
    let month = shiftedMonth < 10 ? shiftedMonth + 3 : shiftedMonth - 9
    let year = yearOfEra + era * 400 + (month <= 2 ? 1 : 0)

    var text = ""
    text.reserveCapacity(24)
    text += padded(year, 4)
    text += "-"
    text += padded(month, 2)
    text += "-"
    text += padded(day, 2)
    text += "T"
    text += padded(millisOfDay / 3_600_000, 2)
    text += ":"
    text += padded(millisOfDay / 60_000 % 60, 2)
    text += ":"
    text += padded(millisOfDay / 1_000 % 60, 2)
    text += "."
    text += padded(millisOfDay % 1_000, 3)
    text += "Z"
    return text
}

func floorDiv(_ a: Int64, _ b: Int64) -> Int64 {
    let quotient = a / b
    return (a % b != 0 && (a < 0) != (b < 0)) ? quotient - 1 : quotient
}

func floorMod(_ a: Int64, _ b: Int64) -> Int64 {
    let remainder = a % b
    return (remainder != 0 && (remainder < 0) != (b < 0)) ? remainder + b : remainder
}

private func padded(_ value: Int64, _ width: Int) -> String {
    let digits = String(value)
    guard digits.count < width else { return digits }
    return String(repeating: "0", count: width - digits.count) + digits
}
