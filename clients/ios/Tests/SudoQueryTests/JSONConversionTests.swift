import CoreGraphics
import XCTest
@testable import SudoQuery

final class JSONConversionTests: XCTestCase {
    private let log = RecordingLog()

    private enum Plan: String {
        case pro = "PRO"
    }

    private enum Tier {
        case gold
    }

    private func encoded(_ object: JSONObject) -> String {
        String(decoding: JSON.encode(.object(object)), as: UTF8.self)
    }

    func testConvertsNestedValuesWithSortedKeys() throws {
        let missing: String? = nil
        let object = JSONConversion.object(
            from: [
                "string": "a",
                "int": 1,
                "int64": Int64(2),
                "uint8": UInt8(7),
                "double": 1.5,
                "decimal": try XCTUnwrap(Decimal(string: "10.25")),
                "bool": true,
                "null": missing as Any,
                "character": Character("x"),
                "raw": Plan.pro,
                "plain": Tier.gold,
                "list": [1, "two"] as [Any],
                "ints": [1, 2],
                "nested": ["inner": ["deep": 1]],
            ],
            log: log
        )
        XCTAssertEqual(
            encoded(object),
            #"{"bool":true,"character":"x","decimal":10.25,"double":1.5,"int":1,"int64":2,"ints":[1,2],"list":[1,"two"],"#
                + #""nested":{"inner":{"deep":1}},"null":null,"plain":"gold","raw":"PRO","string":"a","uint8":7}"#
        )
        XCTAssertEqual(log.warnings, [])
    }

    func testNonFiniteNumbersBecomeNullWithAWarning() {
        let object = JSONConversion.object(from: ["nan": Double.nan, "inf": Float.infinity, "list": [1.0, -Double.infinity]], log: log)
        XCTAssertEqual(encoded(object), #"{"inf":null,"list":[1.0,null],"nan":null}"#)
        XCTAssertEqual(log.warnings.count, 3)
    }

    func testNSNumbersKeepTheirType() {
        // A bridged NSNumber also casts to Bool and Int, so the order of checks matters.
        let object = JSONConversion.object(
            fromAnyKeys: [
                "true": NSNumber(value: true),
                "one": NSNumber(value: 1),
                "zero": NSNumber(value: 0),
                "two": NSNumber(value: 2.0),
                "float": NSNumber(value: Float(0.1)),
                "big": NSNumber(value: UInt64.max),
                "min": NSNumber(value: Int64.min),
                "decimal": NSDecimalNumber(string: "10.25"),
                "notANumber": NSDecimalNumber.notANumber,
                "swiftTrue": true,
                "swiftOne": 1,
                "cgfloat": CGFloat(1.5),
            ],
            log: log
        )
        XCTAssertEqual(
            encoded(object),
            #"{"big":18446744073709551615,"cgfloat":1.5,"decimal":10.25,"float":0.1,"min":-9223372036854775808,"#
                + #""notANumber":null,"one":1,"swiftOne":1,"swiftTrue":true,"true":true,"two":2.0,"zero":0}"#
        )
    }

    func testBoxedOptionalsAreUnwrapped() {
        let some: Int? = 5
        let none: Int? = nil
        let nested: Int?? = .some(nil)
        let object = JSONConversion.object(from: ["some": some as Any, "none": none as Any, "nested": nested as Any], log: log)
        XCTAssertEqual(encoded(object), #"{"nested":null,"none":null,"some":5}"#)
        XCTAssertEqual(JSONConversion.value(nil, log: log), .null)
    }

    func testADictionaryContainingItselfStopsAtTheDepthLimit() throws {
        let loop = NSMutableDictionary()
        loop["self"] = loop
        defer { loop.removeAllObjects() }

        let object = JSONConversion.object(fromAnyKeys: try XCTUnwrap(loop as? [AnyHashable: Any]), log: log)
        var depth = 0
        var current = JSONValue.object(object)
        while case .object(let inner) = current {
            current = inner["self"] ?? .string("missing")
            depth += 1
        }
        XCTAssertEqual(current, .null)
        XCTAssertEqual(depth, 33)
        XCTAssertEqual(log.warnings.count, 1)
        XCTAssertTrue(log.warnings.first?.contains("nested deeper") ?? false)
    }

    func testTheResultIsACopy() throws {
        let items = NSMutableArray(array: ["a"])
        let properties = NSMutableDictionary(dictionary: ["items": items])
        let object = JSONConversion.object(fromAnyKeys: try XCTUnwrap(properties as? [AnyHashable: Any]), log: log)
        items.add("b")
        properties["late"] = 1
        XCTAssertEqual(encoded(object), #"{"items":["a"]}"#)
    }

    func testDictionariesWithNumberKeysGetStringKeys() {
        let object = JSONConversion.object(fromAnyKeys: [NSNumber(value: 1): "one", "b": ["nested": [NSNumber(value: 2): true]]], log: log)
        XCTAssertEqual(encoded(object), #"{"1":"one","b":{"nested":{"2":true}}}"#)
    }

    func testDatesURLsUUIDsSetsAndData() throws {
        let object = JSONConversion.object(
            from: [
                "date": Date(timeIntervalSince1970: 1_760_000_000.5),
                "url": try XCTUnwrap(URL(string: "https://example.com/a?b=c")),
                "uuid": try XCTUnwrap(UUID(uuidString: "3B241101-E2BB-4255-8CAF-4136C566A962")),
                "set": Set(["b", "a"]),
                "data": Data([1, 255]),
            ],
            log: log
        )
        XCTAssertEqual(
            encoded(object),
            #"{"data":[1,255],"date":"2025-10-09T08:53:20.500Z","set":["a","b"],"url":"https://example.com/a?b=c","#
                + #""uuid":"3b241101-e2bb-4255-8caf-4136c566a962"}"#
        )
    }

    func testPlainTurnsJSONBackIntoFoundationValues() {
        let object = JSONObject([
            ("s", .string("x")),
            ("n", .number("3")),
            ("d", .number("2.5")),
            ("b", .bool(true)),
            ("null", .null),
            ("obj", .object(JSONObject([("k", .array([.number("1")]))]))),
        ])
        let plain = JSONConversion.plain(object)
        XCTAssertEqual(plain["s"] as? String, "x")
        XCTAssertEqual(plain["n"] as? Int, 3)
        XCTAssertEqual(plain["d"] as? Double, 2.5)
        XCTAssertEqual(plain["b"] as? Bool, true)
        XCTAssertTrue(plain["null"] is NSNull)
        XCTAssertEqual((plain["obj"] as? [String: Any])?["k"] as? [Int], [1])
    }
}
