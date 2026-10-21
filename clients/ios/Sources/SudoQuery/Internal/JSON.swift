import Foundation

/// A JSON value that can cross queues. A number keeps the literal it will be written as, so a
/// value's exact text (an integer beyond 2^53, a `Decimal`) survives until it is sent.
enum JSONValue: Sendable, Equatable {
    case null
    case bool(Bool)
    /// A valid JSON number literal, e.g. `42`, `-0.5`, `1e-07`.
    case number(String)
    case string(String)
    case array([JSONValue])
    case object(JSONObject)
}

/// A JSON object that keeps its keys in insertion order. Setting a key that exists replaces its
/// value in place, as in Kotlin's `LinkedHashMap` and JavaScript objects.
struct JSONObject: Sendable, Equatable {
    private(set) var keys: [String] = []
    private var values: [String: JSONValue] = [:]

    init() {}

    init(_ pairs: [(String, JSONValue)]) {
        for (key, value) in pairs {
            self[key] = value
        }
    }

    var isEmpty: Bool {
        keys.isEmpty
    }

    var count: Int {
        keys.count
    }

    subscript(key: String) -> JSONValue? {
        get { values[key] }
        set {
            if let newValue {
                if values.updateValue(newValue, forKey: key) == nil { keys.append(key) }
            } else if values.removeValue(forKey: key) != nil {
                keys.removeAll { $0 == key }
            }
        }
    }

    var pairs: [(key: String, value: JSONValue)] {
        keys.compactMap { key in values[key].map { (key, $0) } }
    }

    /// This object with `other` set over it, like `{...self, ...other}` in JavaScript.
    func merging(_ other: JSONObject) -> JSONObject {
        var result = self
        for (key, value) in other.pairs {
            result[key] = value
        }
        return result
    }
}

/// Writes JSON the way the TS client's `JSON.stringify` and the Android SDK do: keys in the order
/// given, nulls written out, no whitespace, `/` and non-ASCII characters left as they are.
/// `JSONEncoder` and `JSONSerialization` guarantee none of the first three.
enum JSON {
    static func encode(_ value: JSONValue) -> [UInt8] {
        var out: [UInt8] = []
        out.reserveCapacity(256)
        write(value, to: &out)
        return out
    }

    static func write(_ value: JSONValue, to out: inout [UInt8]) {
        switch value {
        case .null:
            out.append(contentsOf: "null".utf8)
        case .bool(let flag):
            out.append(contentsOf: (flag ? "true" : "false").utf8)
        case .number(let literal):
            out.append(contentsOf: literal.utf8)
        case .string(let text):
            writeString(text, to: &out)
        case .array(let items):
            out.append(UInt8(ascii: "["))
            for (index, item) in items.enumerated() {
                if index > 0 { out.append(UInt8(ascii: ",")) }
                write(item, to: &out)
            }
            out.append(UInt8(ascii: "]"))
        case .object(let object):
            out.append(UInt8(ascii: "{"))
            for (index, pair) in object.pairs.enumerated() {
                if index > 0 { out.append(UInt8(ascii: ",")) }
                writeString(pair.key, to: &out)
                out.append(UInt8(ascii: ":"))
                write(pair.value, to: &out)
            }
            out.append(UInt8(ascii: "}"))
        }
    }

    static func writeString(_ text: String, to out: inout [UInt8]) {
        out.append(UInt8(ascii: "\""))
        // Bytes of multi-byte UTF-8 sequences are all >= 0x80, so escaping byte by byte is safe.
        for byte in text.utf8 {
            switch byte {
            case UInt8(ascii: "\""): out.append(contentsOf: #"\""#.utf8)
            case UInt8(ascii: "\\"): out.append(contentsOf: #"\\"#.utf8)
            case 0x08: out.append(contentsOf: #"\b"#.utf8)
            case 0x0C: out.append(contentsOf: #"\f"#.utf8)
            case 0x0A: out.append(contentsOf: #"\n"#.utf8)
            case 0x0D: out.append(contentsOf: #"\r"#.utf8)
            case 0x09: out.append(contentsOf: #"\t"#.utf8)
            case 0x00..<0x20:
                out.append(contentsOf: #"\u00"#.utf8)
                out.append(hexDigits[Int(byte >> 4)])
                out.append(hexDigits[Int(byte & 0x0F)])
            default:
                out.append(byte)
            }
        }
        out.append(UInt8(ascii: "\""))
    }

    private static let hexDigits = Array("0123456789abcdef".utf8)
}
