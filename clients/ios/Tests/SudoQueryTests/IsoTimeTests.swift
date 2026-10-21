import Foundation
import XCTest
@testable import SudoQuery

final class IsoTimeTests: XCTestCase {
    func testFormatsLikeDateToISOString() {
        XCTAssertEqual(formatIsoUtcMillis(epochMillis(2026, 10, 14, 9, 30, 12, 345)), "2026-10-14T09:30:12.345Z")
        XCTAssertEqual(formatIsoUtcMillis(T0), "2025-10-09T08:53:20.000Z")
        XCTAssertEqual(formatIsoUtcMillis(0), "1970-01-01T00:00:00.000Z")
        XCTAssertEqual(formatIsoUtcMillis(-1), "1969-12-31T23:59:59.999Z")
    }

    func testHandlesLeapDaysAndYearEnds() {
        XCTAssertEqual(formatIsoUtcMillis(epochMillis(2000, 2, 29, 12, 0, 0, 1)), "2000-02-29T12:00:00.001Z")
        XCTAssertEqual(formatIsoUtcMillis(epochMillis(2024, 2, 29, 23, 59, 59, 999)), "2024-02-29T23:59:59.999Z")
        XCTAssertEqual(formatIsoUtcMillis(epochMillis(2100, 3, 1, 0, 0, 0, 0)), "2100-03-01T00:00:00.000Z")
        XCTAssertEqual(formatIsoUtcMillis(epochMillis(2026, 12, 31, 23, 59, 59, 999)), "2026-12-31T23:59:59.999Z")
        XCTAssertEqual(formatIsoUtcMillis(epochMillis(2027, 1, 1, 0, 0, 0, 0)), "2027-01-01T00:00:00.000Z")
    }

    func testMatchesGmtimeAcrossTwoCenturies() {
        var generator = SplitMix64(seed: 42)
        let end = epochMillis(2170, 1, 1, 0, 0, 0, 0)
        for _ in 0..<20_000 {
            let millis = Int64.random(in: 0..<end, using: &generator)
            XCTAssertEqual(formatIsoUtcMillis(millis), reference(millis))
        }
    }

    func testExtremeValuesDoNotTrap() {
        XCTAssertFalse(formatIsoUtcMillis(.max).isEmpty)
        XCTAssertFalse(formatIsoUtcMillis(.min).isEmpty)
    }

    /// The C library's answer, whole seconds from `gmtime_r` plus the milliseconds.
    private func reference(_ millis: Int64) -> String {
        var seconds = time_t(floorDiv(millis, 1000))
        var parts = tm()
        gmtime_r(&seconds, &parts)
        return String(
            format: "%04d-%02d-%02dT%02d:%02d:%02d.%03dZ",
            parts.tm_year + 1900, parts.tm_mon + 1, parts.tm_mday, parts.tm_hour, parts.tm_min, parts.tm_sec,
            Int32(floorMod(millis, 1000))
        )
    }

    private func epochMillis(_ year: Int32, _ month: Int32, _ day: Int32, _ hour: Int32, _ minute: Int32, _ second: Int32, _ millis: Int64) -> Int64 {
        var parts = tm()
        parts.tm_year = year - 1900
        parts.tm_mon = month - 1
        parts.tm_mday = day
        parts.tm_hour = hour
        parts.tm_min = minute
        parts.tm_sec = second
        return Int64(timegm(&parts)) * 1000 + millis
    }
}
