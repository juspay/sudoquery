import Foundation

/// Who events belong to. Persisted so the anonymous id, user and session survive app restarts.
/// hyper-sdk-ios keeps its session id in UserDefaults; a file needs no privacy-manifest reason.
struct IdentityState: Codable, Equatable, Sendable {
    var anonId: String
    var actorId: String?
    var sessionId: String?
    var lastActivityMillis: Int64

    enum CodingKeys: String, CodingKey {
        case anonId = "anon_id"
        case actorId = "actor_id"
        case sessionId = "session_id"
        case lastActivityMillis = "last_activity_ms"
    }

    init(anonId: String, actorId: String? = nil, sessionId: String? = nil, lastActivityMillis: Int64 = 0) {
        self.anonId = anonId
        self.actorId = actorId
        self.sessionId = sessionId
        self.lastActivityMillis = lastActivityMillis
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        anonId = try container.decode(String.self, forKey: .anonId)
        actorId = try container.decodeIfPresent(String.self, forKey: .actorId)
        sessionId = try container.decodeIfPresent(String.self, forKey: .sessionId)
        lastActivityMillis = try container.decodeIfPresent(Int64.self, forKey: .lastActivityMillis) ?? 0
    }

    /// Applies the session rules for an event at `nowMillis`: a session ends after `timeoutMillis`
    /// without events. If the clock moved backwards the session continues, timed from the new
    /// clock. A `fixedSessionId` from the config is used as is.
    func touched(at nowMillis: Int64, timeoutMillis: Int64, fixedSessionId: String?, newId: () -> String) -> IdentityState {
        let session: String
        if let fixedSessionId {
            session = fixedSessionId
        } else if let sessionId, !Self.timedOut(nowMillis, since: lastActivityMillis, timeoutMillis) {
            session = sessionId
        } else {
            session = newId()
        }
        var state = self
        state.sessionId = session
        state.lastActivityMillis = nowMillis
        return state
    }

    private static func timedOut(_ nowMillis: Int64, since lastMillis: Int64, _ timeoutMillis: Int64) -> Bool {
        let (gap, overflow) = nowMillis.subtractingReportingOverflow(lastMillis)
        return overflow || gap > timeoutMillis
    }
}

/// What `IdentityStore.load()` found.
enum StoredIdentity: Equatable {
    case loaded(IdentityState)
    /// No identity file: a first launch, or the app's data was cleared.
    case missing
    /// The file isn't a valid identity; it gets replaced.
    case corrupt
    /// The file exists but can't be read now, e.g. before the first unlock after a reboot. It must
    /// not be replaced: it holds the anonymous id and user of earlier runs.
    case unavailable
}

/// Keeps `IdentityState` in one small JSON file, replaced atomically so a crash can't leave it
/// half-written.
final class IdentityStore: Sendable {
    private let url: URL
    private let log: any SDKLog

    init(url: URL, log: any SDKLog) {
        self.url = url
        self.log = log
    }

    func load() -> StoredIdentity {
        guard FileIO.exists(url) else { return .missing }
        let data: Data
        do {
            data = try FileIO.readAll(url)
        } catch {
            return .unavailable
        }
        do {
            return .loaded(try JSONDecoder().decode(IdentityState.self, from: data))
        } catch {
            log.warn("Ignoring unreadable \(url.lastPathComponent); starting with a new anonymous id", error: error)
            return .corrupt
        }
    }

    /// Returns false if the identity could not be saved.
    @discardableResult
    func save(_ state: IdentityState) -> Bool {
        do {
            try FileIO.ensureDirectory(url.deletingLastPathComponent())
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.sortedKeys]
            try FileIO.atomicWrite(Array(encoder.encode(state)), to: url)
            return true
        } catch {
            log.warn("Could not save the SDK identity", error: error)
            return false
        }
    }
}
