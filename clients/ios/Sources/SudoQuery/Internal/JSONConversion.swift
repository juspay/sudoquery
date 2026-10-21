import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif

/// Converts values passed to `track()` and `setSuperProperty()` into JSON, on the caller's thread.
/// The result is a copy, so a caller changing its collections afterwards can't alter an event that
/// is already queued.
enum JSONConversion {
    /// Deeper values become null; this also stops a dictionary that contains itself.
    static let maxDepth = 32

    static func object(from dictionary: [String: Any], log: any SDKLog) -> JSONObject {
        convertDictionary(dictionary.map { ($0.key, $0.value) }, depth: 0, log: log)
    }

    /// For dictionaries from Objective-C, whose keys may be numbers.
    static func object(fromAnyKeys dictionary: [AnyHashable: Any], log: any SDKLog) -> JSONObject {
        convertDictionary(dictionary.map { (key(for: $0.key), $0.value) }, depth: 0, log: log)
    }

    static func value(_ value: Any?, log: any SDKLog) -> JSONValue {
        guard let value else { return .null }
        return convert(value, depth: 0, log: log)
    }

    /// The reverse, for handing super properties back to the app: objects become dictionaries,
    /// arrays become arrays and null becomes `NSNull`.
    static func plain(_ value: JSONValue) -> Any {
        switch value {
        case .null:
            return NSNull()
        case .bool(let flag):
            return flag
        case .number(let literal):
            if let integer = Int(literal) { return integer }
            if let unsigned = UInt64(literal) { return unsigned }
            return Double(literal) ?? NSDecimalNumber(string: literal)
        case .string(let text):
            return text
        case .array(let items):
            return items.map(plain)
        case .object(let object):
            return plain(object)
        }
    }

    static func plain(_ object: JSONObject) -> [String: Any] {
        var result: [String: Any] = [:]
        for (key, value) in object.pairs {
            result[key] = plain(value)
        }
        return result
    }

    private static func convert(_ raw: Any, depth: Int, log: any SDKLog) -> JSONValue {
        guard let value = unwrapped(raw) else { return .null }
        if depth > maxDepth {
            log.warn("A property nested deeper than \(maxDepth) levels was replaced with null")
            return .null
        }
        if let json = value as? JSONValue { return json }
        if value is NSNull { return .null }
        // Real NSNumber objects go first: a bridged NSNumber also casts to Bool, Int and Double, so
        // `@1` would become `true` and `@2.0` the integer 2.
        if type(of: value) is NSNumber.Type, let number = value as? NSNumber {
            return convert(number, log: log)
        }
        switch value {
        case let flag as Bool:
            return .bool(flag)
        case let text as String:
            return .string(text)
        case let text as Substring:
            return .string(String(text))
        case let character as Character:
            return .string(String(character))
        case let integer as any BinaryInteger:
            return .number("\(integer)")
        case let number as Double:
            return finite(number.isFinite, number.description, log)
        case let number as Float:
            return finite(number.isFinite, number.description, log)
        case let number as CGFloat:
            return finite(number.isFinite, number.description, log)
        case let number as Decimal:
            return number.isNaN ? nonFinite("NaN", log) : .number(number.description)
        case let date as Date:
            return .string(formatIsoUtcMillis(millis(date.timeIntervalSince1970)))
        case let url as URL:
            return .string(url.absoluteString)
        case let uuid as UUID:
            return .string(uuid.uuidString.lowercased())
        case let data as Data:
            return .array(data.map { .number(String($0)) })
        case let dictionary as [String: Any]:
            return .object(convertDictionary(dictionary.map { ($0.key, $0.value) }, depth: depth, log: log))
        case let dictionary as [AnyHashable: Any]:
            return .object(convertDictionary(dictionary.map { (key(for: $0.key), $0.value) }, depth: depth, log: log))
        case let array as [Any]:
            return .array(array.map { convert($0, depth: depth + 1, log: log) })
        case let set as Set<AnyHashable>:
            return .array(sortedForOutput(set.map { convert($0.base, depth: depth + 1, log: log) }))
        case let set as NSOrderedSet:
            return .array(set.array.map { convert($0, depth: depth + 1, log: log) })
        case let enumValue as any RawRepresentable:
            return convert(enumValue.rawValue, depth: depth + 1, log: log)
        default:
            return .string(String(describing: value))
        }
    }

    private static func convert(_ number: NSNumber, log: any SDKLog) -> JSONValue {
        if CFGetTypeID(number) == CFBooleanGetTypeID() {
            return .bool(number.boolValue)
        }
        if let decimal = number as? NSDecimalNumber {
            return decimal.decimalValue.isNaN ? nonFinite("NaN", log) : .number(decimal.stringValue)
        }
        if CFNumberIsFloatType(number as CFNumber) {
            switch CFNumberGetType(number as CFNumber) {
            case .float32Type, .floatType:
                let value = number.floatValue
                return finite(value.isFinite, value.description, log)
            default:
                let value = number.doubleValue
                return finite(value.isFinite, value.description, log)
            }
        }
        // `stringValue` keeps UInt64 values above Int64.max exact.
        return .number(number.stringValue)
    }

    private static func convertDictionary(_ entries: [(String, Any)], depth: Int, log: any SDKLog) -> JSONObject {
        // Swift dictionaries have no order; sorted keys make the same input give the same bytes.
        var object = JSONObject()
        for (key, value) in entries.sorted(by: { $0.0 < $1.0 }) {
            object[key] = convert(value, depth: depth + 1, log: log)
        }
        return object
    }

    private static func key(for key: AnyHashable) -> String {
        if let text = key.base as? String { return text }
        return String(describing: key.base)
    }

    private static func sortedForOutput(_ values: [JSONValue]) -> [JSONValue] {
        values
            .map { (String(decoding: JSON.encode($0), as: UTF8.self), $0) }
            .sorted { $0.0 < $1.0 }
            .map(\.1)
    }

    /// JSON has no NaN or Infinity; sending one would make the collector reject the whole batch.
    private static func finite(_ isFinite: Bool, _ literal: String, _ log: any SDKLog) -> JSONValue {
        isFinite ? .number(literal) : nonFinite(literal, log)
    }

    private static func nonFinite(_ literal: String, _ log: any SDKLog) -> JSONValue {
        log.warn("A property value of \(literal) can't be sent as JSON and was replaced with null")
        return .null
    }

    /// Optionals boxed in `Any`, such as `["coupon": coupon as Any]`, unwrapped or null.
    private static func unwrapped(_ value: Any) -> Any? {
        var current = value
        while let optional = current as? AnyOptional {
            guard let wrapped = optional.wrappedAny else { return nil }
            current = wrapped
        }
        return current
    }
}

private protocol AnyOptional {
    var wrappedAny: Any? { get }
}

extension Optional: AnyOptional {
    fileprivate var wrappedAny: Any? {
        switch self {
        case .some(let wrapped): return wrapped
        case .none: return nil
        }
    }
}
