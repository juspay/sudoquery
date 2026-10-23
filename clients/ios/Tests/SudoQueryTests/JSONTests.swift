import XCTest
@testable import SudoQuery

final class JSONTests: XCTestCase {
    private func encoded(_ value: JSONValue) -> String {
        String(decoding: JSON.encode(value), as: UTF8.self)
    }

    func testStringsAreEscapedLikeJSONStringify() {
        XCTAssertEqual(
            encoded(.string("quote \" backslash \\ slash / tab \t newline \n return \r backspace \u{8} formfeed \u{C} control \u{1} \u{1F}")),
            #""quote \" backslash \\ slash / tab \t newline \n return \r backspace \b formfeed \f control \u0001 \u001f""#
        )
        XCTAssertEqual(encoded(.string("₹ 中文 😀 \u{2028} \u{7F}")), "\"₹ 中文 😀 \u{2028} \u{7F}\"")
    }

    func testObjectsKeepInsertionOrderAndWriteNulls() {
        var object = JSONObject([("b", .number("1")), ("a", .null)])
        object["b"] = .bool(true) // replaced in place
        object["c"] = .array([.string("x"), .number("-0.5"), .object(JSONObject())])
        XCTAssertEqual(encoded(.object(object)), #"{"b":true,"a":null,"c":["x",-0.5,{}]}"#)

        object["a"] = nil
        XCTAssertEqual(object.keys, ["b", "c"])
        XCTAssertEqual(object.merging(JSONObject([("c", .null), ("d", .string("new"))])).keys, ["b", "c", "d"])
    }

    func testEncodedStringsRoundTripThroughJSONSerialization() throws {
        var generator = SplitMix64(seed: 7)
        for _ in 0..<500 {
            let count = Int.random(in: 0...24, using: &generator)
            let scalars = (0..<count).compactMap { _ in Unicode.Scalar(UInt32.random(in: 0...0x2FFF, using: &generator)) }
            let text = String(String.UnicodeScalarView(scalars))
            let parsed = try JSONSerialization.jsonObject(with: Data(JSON.encode(.array([.string(text)])))) as? [String]
            XCTAssertEqual(parsed, [text])
        }
    }

    func testNumberLiteralsAreValidJSONAndRoundTrip() throws {
        var generator = SplitMix64(seed: 11)
        var doubles: [Double] = [0, -0.0, 1, 1.5, 0.1, 1e16, 1e-7, 123_456_789, 5e-324, .greatestFiniteMagnitude, -2.5e-300]
        for _ in 0..<2_000 {
            doubles.append(Double(bitPattern: generator.next()))
        }
        for value in doubles where value.isFinite {
            let literal = value.description
            // Valid JSON (JSONSerialization's own parsing may be an ULP off, so it isn't the judge of exactness)...
            let data = Data(JSON.encode(.array([.number(literal)])))
            let parsed = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [NSNumber], literal)
            XCTAssertEqual(parsed.first?.doubleValue ?? .nan, value, accuracy: abs(value) * 1e-15, literal)
            // ...and exact for a correctly rounding parser, such as serde_json's with float_roundtrip.
            XCTAssertEqual(Double(literal), value, literal)
        }
        for value in [Float(0.1), .greatestFiniteMagnitude, .leastNormalMagnitude, .leastNonzeroMagnitude] {
            let data = Data(JSON.encode(.array([.number(value.description)])))
            XCTAssertNoThrow(try JSONSerialization.jsonObject(with: data), value.description)
        }
    }
}
