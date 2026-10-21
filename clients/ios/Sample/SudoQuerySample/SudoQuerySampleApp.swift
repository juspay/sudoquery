import SudoQuery
import SwiftUI

@main
struct SudoQuerySampleApp: App {
    init() {
        SampleSDK.start()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

enum SampleSDK {
    /// The endpoint, org and project come from the SUDOQUERY_* build settings (through Info.plist);
    /// environment variables of the same names override them, e.g. with
    /// `SIMCTL_CHILD_SUDOQUERY_ENDPOINT=… xcrun simctl launch …`.
    static func start() {
        let info = Bundle.main.infoDictionary ?? [:]
        let environment = ProcessInfo.processInfo.environment
        func setting(_ name: String, _ key: String) -> String {
            environment[name] ?? (info[key] as? String) ?? ""
        }
        do {
            let config = try SudoQueryConfig(
                orgId: setting("SUDOQUERY_ORG_ID", "SudoQueryOrgId"),
                projectId: setting("SUDOQUERY_PROJECT_ID", "SudoQueryProjectId"),
                endpoint: setting("SUDOQUERY_ENDPOINT", "SudoQueryEndpoint"),
                batchSize: 20,
                flushInterval: 10,
                debugLogging: true
            )
            SudoQuery.initialize(config)
            SudoQuery.setSuperProperty("app_version", (info["CFBundleShortVersionString"] as? String) ?? "dev")
        } catch {
            print("SudoQuery is not configured: \(error)")
        }
        SmokeTest.runIfRequested(ProcessInfo.processInfo.arguments)
    }
}
