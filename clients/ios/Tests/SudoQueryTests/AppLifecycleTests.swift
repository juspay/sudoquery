#if canImport(UIKit) && !os(watchOS)
import UIKit
import XCTest
@testable import SudoQuery

final class AppLifecycleTests: XCTestCase {
    private final class FakeBackgroundTasks: BackgroundTasks {
        private let state = Locked((next: 0, begun: [Int](), ended: [Int](), expirations: [@Sendable () -> Void]()))

        var begun: [Int] { state.current.begun }
        var ended: [Int] { state.current.ended }

        func begin(name: String, expiration: @escaping @Sendable () -> Void) -> UIBackgroundTaskIdentifier? {
            state.withLock { state in
                state.next += 1
                state.begun.append(state.next)
                state.expirations.append(expiration)
                return UIBackgroundTaskIdentifier(rawValue: state.next)
            }
        }

        func end(_ identifier: UIBackgroundTaskIdentifier) {
            state.withLock { $0.ended.append(identifier.rawValue) }
        }

        func expire(_ index: Int) {
            state.current.expirations[index]()
        }
    }

    private let center = NotificationCenter()
    private let tasks = FakeBackgroundTasks()
    private let flushes = Locked<[@Sendable () -> Void]>([])
    private let foregrounds = Locked(0)
    private var observer: AppLifecycleObserver!

    override func setUp() {
        super.setUp()
        observer = makeObserver(tasks: { [tasks] in tasks })
    }

    override func tearDown() {
        observer.unregister()
        super.tearDown()
    }

    private func makeObserver(tasks: @escaping @Sendable () -> (any BackgroundTasks)?) -> AppLifecycleObserver {
        let observer = AppLifecycleObserver(
            center: center,
            tasks: tasks,
            onBackground: { [flushes] done in flushes.withLock { $0.append(done) } },
            onForeground: { [foregrounds] in foregrounds.withLock { $0 += 1 } }
        )
        observer.register()
        return observer
    }

    private func enterBackground() {
        center.post(name: UIApplication.didEnterBackgroundNotification, object: nil)
    }

    func testGoingToTheBackgroundBeginsATaskAndEndsItOnceWhenTheFlushIsDone() {
        enterBackground()
        XCTAssertEqual(tasks.begun, [1])
        XCTAssertEqual(flushes.current.count, 1)
        XCTAssertEqual(tasks.ended, [])

        flushes.current[0]()
        flushes.current[0]()
        XCTAssertEqual(tasks.ended, [1])
    }

    func testAnExpiredTaskIsEndedAndNotEndedAgain() {
        enterBackground()
        tasks.expire(0)
        XCTAssertEqual(tasks.ended, [1])
        flushes.current[0]()
        XCTAssertEqual(tasks.ended, [1])
    }

    func testASecondTripToTheBackgroundEndsTheFirstTask() {
        enterBackground()
        enterBackground()
        XCTAssertEqual(tasks.begun, [1, 2])
        XCTAssertEqual(tasks.ended, [1])
        flushes.current[1]()
        XCTAssertEqual(tasks.ended, [1, 2])
    }

    func testComingToTheForegroundIsReported() {
        center.post(name: UIApplication.willEnterForegroundNotification, object: nil)
        XCTAssertEqual(foregrounds.current, 1)
    }

    func testWithoutAnApplicationTheBatchIsStillFlushed() {
        observer.unregister()
        observer = makeObserver(tasks: { nil })
        enterBackground()
        XCTAssertEqual(flushes.current.count, 1)
        flushes.current[0]()
        XCTAssertEqual(tasks.begun, [])
    }

    func testUnregisteredObserversHearNothing() {
        observer.unregister()
        enterBackground()
        XCTAssertEqual(flushes.current.count, 0)
    }
}
#endif
