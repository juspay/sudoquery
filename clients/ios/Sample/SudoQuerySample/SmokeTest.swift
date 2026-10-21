import SudoQuery

/// Launch arguments for checking the sample from the command line:
///
/// ```
/// xcrun simctl launch <device> io.juspay.sudoquery.sample --sudoquery-smoke-test
/// ```
///
/// `--sudoquery-smoke-test` tracks three `smoke_test` events and flushes them;
/// `--sudoquery-objc-smoke` runs ObjCSmokeTest.m, which tracks an `objc_smoke` event;
/// `--sudoquery-background-test` tracks two `background_test` events without flushing, so they
/// go out when the app is sent to the background.
enum SmokeTest {
    static func runIfRequested(_ arguments: [String]) {
        if arguments.contains("--sudoquery-smoke-test") {
            for index in 0..<3 {
                SudoQuery.track("smoke_test", properties: ["index": index])
            }
            SudoQuery.flush()
        }
        if arguments.contains("--sudoquery-background-test") {
            for index in 0..<2 {
                SudoQuery.track("background_test", properties: ["index": index])
            }
        }
        if arguments.contains("--sudoquery-objc-smoke") {
            SQSampleRunObjCSmokeTest()
        }
    }
}
