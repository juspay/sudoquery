import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// File operations for the event queue and the identity file, ported from Airborne's `AJPFileUtil`
/// (atomic writes, folder creation, deletes).
///
/// Unlike there, the files live in Application Support, excluded from backup, with an explicit
/// protection class; folders are created once instead of on every call; and failures are thrown,
/// not swallowed by `try?`. Plain POSIX calls are used so a failure is always a Swift error (never
/// an Objective-C exception such as `FileHandle.write` raises) and an event is one `write`.
enum FileIO {
    struct Failure: Error, CustomStringConvertible, LocalizedError {
        let operation: String
        let name: String
        let code: Int32

        var description: String {
            "\(operation) \(name) failed: \(String(cString: strerror(code)))"
        }

        var errorDescription: String? {
            description
        }
    }

    /// Readable after the first unlock since boot, so a flush in the background still works while
    /// the device is locked. Apps whose default class is `complete` would otherwise lose that.
    static var protectionAttributes: [FileAttributeKey: Any]? {
        #if os(iOS) || os(tvOS) || os(watchOS) || os(visionOS)
        return [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]
        #else
        return nil
        #endif
    }

    /// Creates `url` and any missing parents.
    static func ensureDirectory(_ url: URL) throws {
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true, attributes: protectionAttributes)
    }

    /// Best effort: the directory's class normally applies already.
    static func setProtection(_ url: URL) {
        guard let attributes = protectionAttributes else { return }
        try? FileManager.default.setAttributes(attributes, ofItemAtPath: url.path)
    }

    /// Keeps `url` (and everything in it) out of iCloud and Finder backups, so the anonymous id is
    /// never restored onto another device.
    static func excludeFromBackup(_ url: URL) throws {
        var url = url
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try url.setResourceValues(values)
    }

    static func openForAppend(_ url: URL) throws -> Int32 {
        try openFile(url, flags: O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC)
    }

    /// Writes all of `bytes`, retrying short writes and interruptions.
    static func writeAll(_ fd: Int32, _ bytes: [UInt8], name: String) throws {
        try bytes.withUnsafeBytes { buffer in
            guard var pointer = buffer.baseAddress else { return }
            var remaining = buffer.count
            while remaining > 0 {
                let written = Darwin.write(fd, pointer, remaining)
                if written < 0 {
                    let code = errno
                    if code == EINTR { continue }
                    throw Failure(operation: "write", name: name, code: code)
                }
                pointer = pointer.advanced(by: written)
                remaining -= written
            }
        }
    }

    static func closeFile(_ fd: Int32, name: String) throws {
        if Darwin.close(fd) != 0 {
            throw Failure(operation: "close", name: name, code: errno)
        }
    }

    /// `rename(2)`: atomic, and replaces `destination` if it exists.
    static func moveFile(_ source: URL, to destination: URL) throws {
        var code: Int32 = 0
        let result = source.withUnsafeFileSystemRepresentation { from in
            destination.withUnsafeFileSystemRepresentation { to -> Int32 in
                guard let from, let to else {
                    code = EINVAL
                    return -1
                }
                let result = Darwin.rename(from, to)
                if result != 0 { code = errno }
                return result
            }
        }
        if result != 0 {
            throw Failure(operation: "rename", name: source.lastPathComponent, code: code)
        }
    }

    /// Returns true if the file was deleted.
    @discardableResult
    static func removeFile(_ url: URL) -> Bool {
        url.withUnsafeFileSystemRepresentation { path in
            guard let path else { return false }
            return unlink(path) == 0
        }
    }

    static func exists(_ url: URL) -> Bool {
        FileManager.default.fileExists(atPath: url.path)
    }

    /// The file's size. `URLResourceValues` rather than `stat`, which the privacy manifest would
    /// have to declare as a file-timestamp API.
    static func fileSize(_ url: URL) -> Int64? {
        guard let size = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize else { return nil }
        return Int64(size)
    }

    static func listNames(_ directory: URL) -> [String] {
        (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []
    }

    static func readAll(_ url: URL) throws -> Data {
        try Data(contentsOf: url)
    }

    /// Writes `bytes` to a temporary file, syncs it and renames it over `url`, so a crash leaves
    /// either the old file or the new one, never half of one.
    static func atomicWrite(_ bytes: [UInt8], to url: URL) throws {
        let temp = url.deletingLastPathComponent().appendingPathComponent(url.lastPathComponent + ".tmp")
        let fd = try openFile(temp, flags: O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC)
        do {
            try writeAll(fd, bytes, name: temp.lastPathComponent)
            if fsync(fd) != 0 {
                throw Failure(operation: "fsync", name: temp.lastPathComponent, code: errno)
            }
        } catch {
            _ = Darwin.close(fd)
            removeFile(temp)
            throw error
        }
        try closeFile(fd, name: temp.lastPathComponent)
        try moveFile(temp, to: url)
    }

    private static func openFile(_ url: URL, flags: Int32) throws -> Int32 {
        var code: Int32 = 0
        let fd = url.withUnsafeFileSystemRepresentation { path -> Int32 in
            guard let path else {
                code = EINVAL
                return -1
            }
            let fd = open(path, flags, 0o600)
            if fd < 0 { code = errno }
            return fd
        }
        guard fd >= 0 else {
            throw Failure(operation: "open", name: url.lastPathComponent, code: code)
        }
        return fd
    }
}
