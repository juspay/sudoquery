import SudoQuery
import SwiftUI

struct ContentView: View {
    @State private var clicks = 0
    @State private var status = "Ready"

    var body: some View {
        NavigationStack {
            List {
                Section("Events") {
                    Button("Track button_click") {
                        clicks += 1
                        SudoQuery.track("button_click", properties: ["clicks": clicks, "screen": "home"])
                        status = "Tracked button_click #\(clicks)"
                    }
                    Button("Track 100 events") {
                        for index in 0..<100 {
                            SudoQuery.track("bulk_event", properties: ["index": index])
                        }
                        status = "Tracked 100 events"
                    }
                }
                Section("User") {
                    Button("Set user user_123") {
                        SudoQuery.setUser("user_123")
                        status = "User is user_123"
                    }
                    Button("Remove user") {
                        SudoQuery.removeUser()
                        status = "User removed"
                    }
                    Button("Reset (logout)") {
                        SudoQuery.reset()
                        status = "Reset: new anonymous id and session"
                    }
                }
                Section("Delivery") {
                    Button("Flush") {
                        SudoQuery.flush()
                        status = "Flushed"
                    }
                    Button("Run Objective-C smoke test") {
                        SQSampleRunObjCSmokeTest()
                        status = "Objective-C smoke test ran; see the log"
                    }
                }
                Section("Status") {
                    Text(status)
                }
            }
            .navigationTitle("SudoQuery \(SudoQuery.version)")
        }
    }
}
