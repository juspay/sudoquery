/// Properties added to every event. In memory only, as in the TS client.
final class SuperProperties: Sendable {
    private let values = Locked(JSONObject())

    func set(_ key: String, _ value: JSONValue) {
        values.withLock { $0[key] = value }
    }

    func clear() {
        values.withLock { $0 = JSONObject() }
    }

    func snapshot() -> JSONObject {
        values.current
    }

    /// The event's own properties win, like `{...superProperties, ...properties}` in the TS client.
    func mergeUnder(_ properties: JSONObject) -> JSONObject {
        let current = snapshot()
        return current.isEmpty ? properties : current.merging(properties)
    }
}
