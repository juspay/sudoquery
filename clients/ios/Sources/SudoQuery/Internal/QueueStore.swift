import Foundation

/// A batch file, named `<seq>-<createdMillis>.ndjson`.
struct BatchFile: Sendable, Equatable {
    let url: URL
    let name: String
    let seq: Int64
    let createdMillis: Int64
}

/// The on-disk event queue, ported from the batch files of hyper-sdk-ios's `HPJPLogPusher`: one
/// immutable file per batch, deleted once the collector acknowledges it, replayed on the next launch.
///
/// Encoded events are appended as NDJSON lines to one open batch file in `staging/`. When the batch
/// is full (or flushed) it is committed: closed and renamed into `ready/`, after which it is never
/// modified. The uploader sends `ready/` files oldest first and deletes each once delivered.
///
/// Unlike `HPJPLogPusher`, a batch becomes visible to the uploader only once it is completely
/// written; the queue is ordered by a sequence number, not by an unsorted directory listing, so a
/// wall-clock change can't reorder it (the creation time is only used for expiry); it has size and
/// age limits; and nothing is wiped when the SDK is upgraded.
///
/// `recover`, `append`, `commit` and `enforceLimits` belong to the writer queue. `readyBatches`,
/// `read` and `delete` are also called by the uploader: committed files never change and deleting
/// one twice is harmless.
final class QueueStore: @unchecked Sendable {
    static let maxBatchBytes: Int64 = 512 * 1024
    static let maxQueueBytes: Int64 = 10 * 1024 * 1024
    static let maxAgeMillis: Int64 = 7 * 24 * millisPerHour
    private static let millisPerHour: Int64 = 60 * 60 * 1000

    private let stagingDir: URL
    private let readyDir: URL
    private let maxEventsPerBatch: Int
    private let maxBytesPerBatch: Int64
    private let maxQueueBytes: Int64
    private let maxAgeMillis: Int64
    private let time: any TimeSource
    private let log: any SDKLog

    // Writer queue only.
    private var nextSeq: Int64 = 1
    private var open: OpenBatch?

    private struct OpenBatch {
        let file: BatchFile
        let fd: Int32
        var events = 0
        var bytes: Int64 = 0
    }

    init(
        root: URL,
        maxEventsPerBatch: Int,
        time: any TimeSource,
        log: any SDKLog,
        maxBytesPerBatch: Int64 = QueueStore.maxBatchBytes,
        maxQueueBytes: Int64 = QueueStore.maxQueueBytes,
        maxAgeMillis: Int64 = QueueStore.maxAgeMillis
    ) {
        stagingDir = root.appendingPathComponent("staging", isDirectory: true)
        readyDir = root.appendingPathComponent("ready", isDirectory: true)
        self.maxEventsPerBatch = maxEventsPerBatch
        self.maxBytesPerBatch = maxBytesPerBatch
        self.maxQueueBytes = maxQueueBytes
        self.maxAgeMillis = maxAgeMillis
        self.time = time
        self.log = log
    }

    /// Commits any batch a previous process left open, and picks the next sequence number. Call
    /// once, first.
    func recover() throws {
        try FileIO.ensureDirectory(stagingDir)
        try FileIO.ensureDirectory(readyDir)
        var recovered = 0
        for name in FileIO.listNames(stagingDir) {
            let url = stagingDir.appendingPathComponent(name)
            if Self.parseName(name) == nil || (FileIO.fileSize(url) ?? 0) == 0 {
                FileIO.removeFile(url)
            } else {
                moveToReady(url, name: name)
                recovered += 1
            }
        }
        if recovered > 0 {
            log.debug("Recovered \(recovered) unsent batch(es) from the previous run")
        }
        nextSeq = (FileIO.listNames(readyDir).compactMap { Self.parseName($0)?.seq }.max() ?? 0) + 1
    }

    /// Why `append` failed, and whether it still committed a batch (the one before the event).
    struct AppendFailure: Error {
        let cause: any Error
        let committed: Bool
    }

    /// Appends one encoded event (without a trailing newline) with a single unbuffered write, so it
    /// reaches the file system even if the process dies right after. Returns true if a batch was
    /// committed, i.e. there is something new to upload. Throws `AppendFailure`.
    func append(_ event: [UInt8]) throws -> Bool {
        var committed = false
        if let current = open, current.bytes + Int64(event.count) + 1 > maxBytesPerBatch {
            committed = commit()
        }

        var record = event
        record.append(UInt8(ascii: "\n"))
        do {
            let batch: OpenBatch
            if let current = open {
                batch = current
            } else {
                batch = try openBatch()
                open = batch
            }
            do {
                try FileIO.writeAll(batch.fd, record, name: batch.file.name)
            } catch {
                // Keep the events already written; the reader skips the partial line.
                committed = commit() || committed
                throw error
            }
        } catch {
            throw AppendFailure(cause: error, committed: committed)
        }
        open?.events += 1
        open?.bytes += Int64(record.count)

        if let events = open?.events, events >= maxEventsPerBatch {
            committed = commit() || committed
        }
        return committed
    }

    /// Closes the open batch and moves it to `ready/`. Returns true if it held any events.
    @discardableResult
    func commit() -> Bool {
        guard let batch = open else { return false }
        open = nil
        do {
            try FileIO.closeFile(batch.fd, name: batch.file.name)
        } catch {
            log.warn("Could not close \(batch.file.name)", error: error)
        }
        if batch.events == 0 {
            FileIO.removeFile(batch.file.url)
            return false
        }
        moveToReady(batch.file.url, name: batch.file.name)
        return true
    }

    /// Committed batches, oldest first.
    func readyBatches() -> [BatchFile] {
        FileIO.listNames(readyDir)
            .compactMap { name in
                Self.parseName(name).map {
                    BatchFile(url: readyDir.appendingPathComponent(name), name: name, seq: $0.seq, createdMillis: $0.createdMillis)
                }
            }
            .sorted { $0.seq < $1.seq }
    }

    /// The events of a committed batch, exactly as they were written, or nil if the file is gone.
    /// Lines that aren't a JSON object, such as a write cut short by a crash, are skipped.
    func read(_ file: BatchFile) -> [Data]? {
        let data: Data
        do {
            data = try FileIO.readAll(file.url)
        } catch {
            if FileIO.exists(file.url) {
                log.warn("Could not read \(file.name)", error: error)
            }
            return nil
        }
        var events: [Data] = []
        var skipped = 0
        for line in data.split(separator: UInt8(ascii: "\n"), omittingEmptySubsequences: true) {
            if line.allSatisfy({ $0 == UInt8(ascii: " ") || $0 == UInt8(ascii: "\t") || $0 == UInt8(ascii: "\r") }) {
                continue
            }
            let event = Data(line)
            if (try? JSONSerialization.jsonObject(with: event)) is [String: Any] {
                events.append(event)
            } else {
                skipped += 1
            }
        }
        if skipped > 0 {
            log.warn("Skipped \(skipped) unreadable event(s) in \(file.name)")
        }
        return events
    }

    func delete(_ file: BatchFile) {
        if !FileIO.removeFile(file.url) && FileIO.exists(file.url) {
            log.warn("Could not delete \(file.name)")
        }
    }

    /// Drops batches older than the maximum age, then the oldest batches while the queue is over
    /// its size limit, down to 80 % of it. A file's size is taken before it is deleted.
    func enforceLimits() {
        let now = time.nowMillis()
        var expired = 0
        var kept: [(file: BatchFile, size: Int64)] = []
        for file in readyBatches() {
            // A batch dated in the future (the clock moved back) counts as fresh.
            if now - file.createdMillis > maxAgeMillis {
                if FileIO.removeFile(file.url) { expired += 1 }
            } else {
                kept.append((file, FileIO.fileSize(file.url) ?? 0))
            }
        }

        var total = kept.reduce(Int64(0)) { $0 + $1.size }
        var trimmed = 0
        if total > maxQueueBytes {
            let target = maxQueueBytes / 10 * 8
            for (file, size) in kept {
                if total <= target { break }
                let removed = FileIO.removeFile(file.url)
                // Gone either way: deleted here, or delivered by the uploader meanwhile.
                if removed || !FileIO.exists(file.url) {
                    total -= size
                    if removed { trimmed += 1 }
                }
            }
        }

        if expired > 0 {
            log.warn("Dropped \(expired) unsent batch(es) older than \(maxAgeMillis / Self.millisPerHour) h")
        }
        if trimmed > 0 {
            log.warn("Dropped the \(trimmed) oldest unsent batch(es) to keep the queue under \(maxQueueBytes) bytes")
        }
    }

    /// Closes the open batch without committing it, as if the process had ended. For tests.
    func closeOpenBatch() {
        guard let batch = open else { return }
        open = nil
        try? FileIO.closeFile(batch.fd, name: batch.file.name)
    }

    private func openBatch() throws -> OpenBatch {
        let seq = nextSeq
        nextSeq += 1
        let created = max(0, time.nowMillis())
        let name = Self.fileName(seq: seq, createdMillis: created)
        let url = stagingDir.appendingPathComponent(name)
        let fd: Int32
        do {
            fd = try FileIO.openForAppend(url)
        } catch let failure as FileIO.Failure where failure.code == ENOENT {
            // The directory was removed while the app ran; create it again.
            try FileIO.ensureDirectory(stagingDir)
            fd = try FileIO.openForAppend(url)
        }
        FileIO.setProtection(url)
        return OpenBatch(file: BatchFile(url: url, name: name, seq: seq, createdMillis: created), fd: fd)
    }

    private func moveToReady(_ url: URL, name: String) {
        let destination = readyDir.appendingPathComponent(name)
        do {
            try FileIO.moveFile(url, to: destination)
        } catch let failure as FileIO.Failure where failure.code == ENOENT && FileIO.exists(url) {
            do {
                try FileIO.ensureDirectory(readyDir)
                try FileIO.moveFile(url, to: destination)
            } catch {
                log.warn("Could not move \(name) to the upload queue", error: error)
            }
        } catch {
            log.warn("Could not move \(name) to the upload queue", error: error)
        }
    }

    static func fileName(seq: Int64, createdMillis: Int64) -> String {
        let digits = String(seq)
        let padded = digits.count < 10 ? String(repeating: "0", count: 10 - digits.count) + digits : digits
        return "\(padded)-\(createdMillis).ndjson"
    }

    static func parseName(_ name: String) -> (seq: Int64, createdMillis: Int64)? {
        let suffix = ".ndjson"
        guard name.hasSuffix(suffix) else { return nil }
        let parts = name.dropLast(suffix.count).split(separator: "-", omittingEmptySubsequences: false)
        guard parts.count == 2, let seq = number(parts[0]), let created = number(parts[1]) else { return nil }
        return (seq, created)
    }

    private static func number(_ text: Substring) -> Int64? {
        guard (1...18).contains(text.count), text.allSatisfy({ ("0"..."9").contains($0) }) else { return nil }
        return Int64(text)
    }
}
