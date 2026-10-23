# SudoQuery iOS SDK

The iOS client for the SudoQuery events collector: the Swift counterpart of
[`clients/typescript`](../typescript) and `clients/android`, usable from Swift and Objective-C.
Track events from anywhere in your app; the SDK batches them, keeps them on disk until they are
delivered, and posts them to the collector's `POST /v1/events/batch` route.

- **Durable:** events are written to disk as they are tracked and survive app restarts and crashes.
- **Batched:** one request per batch, sent when a batch fills up, on a timer, and when the app goes to the background (inside a background task, so the upload can finish).
- **Safe to call anywhere:** every method is thread-safe, returns at once and never throws or traps. Disk and network work runs on the SDK's own background queues, never on the main thread.
- **Small:** one Swift package with no dependencies. It needs no Swift Concurrency runtime, so iOS 13 and 14 apps, Objective-C-only ones included, don't need to ship one.

Its network, batching and file layers are ported from hyper-sdk-ios (`HPJPLogPusher`,
`HPJPLogsManager`, Airborne's `AJPNetworkClient` and `AJPFileUtil`), with that SDK's bugs fixed:
any 2xx counts as delivered; 400, 413 and 422 drop the batch instead of losing every batch on any
error; other failures back off exponentially instead of retrying back to back; a transport error is
never turned into an empty `URLResponse()`; batches are visible to the uploader only once fully
written; and the queue has size and age limits.

## Installation

Requirements: iOS 13 or newer, Xcode 16 or newer (Swift tools 6.0).

The package lives in this monorepo at `clients/ios` and isn't published yet, so add it as a local
package:

- **Xcode:** File → Add Package Dependencies… → Add Local… and pick `clients/ios`, then add the
  `SudoQuery` library to your app target.
- **Package.swift:** `.package(path: "<path to sudoquery>/clients/ios")` and the product `"SudoQuery"`.

Keeping a checkout of the repository next to the app (for example as a git submodule) works for
both. A remote dependency (`.package(url:…)` or Xcode's URL field) can't resolve yet: SwiftPM only
reads a `Package.swift` at the root of a repository (see [Publishing](#publishing)).

## Quick start

Initialize once, as early as possible: in `application(_:didFinishLaunchingWithOptions:)`, or in
your SwiftUI `App`'s `init`:

```swift
import SudoQuery

@main
struct MyApp: App {
    init() {
        do {
            SudoQuery.initialize(try SudoQueryConfig(
                orgId: "acme-store-k3x9qa",
                workspaceId: "acme-store-app-9t2r4m",
                endpoint: "https://collector.example.com/v1/events/batch"
            ))
        } catch {
            print("SudoQuery isn't configured: \(error)")
        }
    }

    var body: some Scene {
        WindowGroup { ContentView() }
    }
}
```

Then track events from anywhere:

```swift
SudoQuery.track("button_click", properties: ["button_id": "submit", "page": "/checkout"])

SudoQuery.setUser("user_123")             // sent as actor_id from now on, and after restarts
SudoQuery.setSuperProperty("plan", "pro") // added to every event
SudoQuery.reset()                         // on logout: new anonymous id and session, no user
```

From Objective-C, enable modules (`CLANG_ENABLE_MODULES`, on by default) and use `SQSudoQuery`:

```objc
@import SudoQuery;

SQSudoQueryConfig *config =
    [[SQSudoQueryConfig alloc] initWithOrgId:@"acme-store-k3x9qa"
                                 workspaceId:@"acme-store-app-9t2r4m"
                                    endpoint:@"https://collector.example.com/v1/events/batch"];
NSError *error = nil;
if (![SQSudoQuery initializeWithConfig:config error:&error]) {
    NSLog(@"SudoQuery isn't configured: %@", error.localizedDescription);
}
[SQSudoQuery track:@"button_click" properties:@{@"button_id": @"submit"}];
```

`orgId` and `workspaceId` are the server-generated slugs of the organization and the project. The
dashboard's project settings show them as Organization ID and Project ID, and its switcher copies
them as the tenant ID and the workspace ID. They are sent as the `x-org-id` / `x-workspace-id`
headers and as every event's `org_id` / `workspace_id`.

## Configuration

`SudoQueryConfig(orgId:workspaceId:endpoint:…)` takes:

| Parameter (Swift / Obj-C property) | Default | Description |
|---|---|---|
| `endpoint` | required | Full URL of the batch route, e.g. `https://collector.example.com/v1/events/batch`. |
| `token` | none | Sent as `Authorization: Bearer <token>`, for a proxy or gateway in front of the collector. |
| `headers` | none | Extra headers. They can't replace `x-org-id`, `x-workspace-id` or the bearer token. |
| `source` | `"ios"` | The `source` of every event. |
| `sessionId` | managed | Use a fixed session id instead of SDK-managed sessions. |
| `batchSize` | `50` | Events per batch and request (1 to 500). |
| `flushInterval` | `30` s | Send pending events at least this often, in seconds; `0` turns the timer off. |
| `sessionTimeout` | `1800` s | A new session starts after this many seconds without events. |
| `debugLogging` | `false` | Debug logs in the unified log (subsystem `io.juspay.sudoquery`). Warnings and errors are always logged. |

The initializer throws `SudoQueryConfigError` for a blank id, an endpoint that isn't an http(s)
URL with a host, an out-of-range value, or a header with a line break. From Objective-C,
`initializeWithConfig:error:` returns `NO` with an `NSError` in the `SQSudoQueryConfig.errorDomain`
domain (`io.juspay.sudoquery.config`) and an `SQSudoQueryConfigErrorCode` code. Ids that don't look
like slugs only log a warning. Intervals are seconds (`TimeInterval`), not milliseconds as on Android.

## API

| Swift (`SudoQuery`) | Objective-C (`SQSudoQuery`) | Description |
|---|---|---|
| `initialize(_:)` | `+initializeWithConfig:error:` | Starts the SDK. The first call wins; later calls are ignored. |
| `track(_:properties:)` | `+track:`, `+track:properties:` | Records an event. Properties are a dictionary; nested dictionaries, arrays and sets are supported. `Date`s become ISO 8601 strings, `URL`s strings and `UUID`s lowercase strings; NaN and infinite numbers become `null`. |
| `setUser(_:)` / `removeUser()` / `user` | `+setUser:` (nil removes) / `+removeUser` / `user` | The user id sent as `actor_id`. It is kept across app restarts. |
| `setSuperProperty(_:_:)` / `superProperties` / `clearSuperProperties()` | `+setSuperProperty:value:` / `superProperties` / `+clearSuperProperties` | Properties added to every event (the event's own keys win). In memory only; usable before `initialize`. |
| `flush()` | `+flush` | Sends the events tracked so far now instead of waiting for a full batch. |
| `reset()` | `+reset` | For logout: later events get a new anonymous id and session, with no user id and no super properties. |
| `isInitialized` | `isInitialized` | True once `initialize` has started the SDK. |
| `version` | `sdkVersion` | The SDK version (`+version` is taken by `NSObject`). |

Events tracked before `initialize` are dropped with a warning. The properties are copied when
`track` is called, so changing a dictionary afterwards doesn't change the event; mutating an
`NSMutableDictionary` from another thread while `track` copies it is the caller's race.

## How events are delivered

1. `track()` copies the properties and timestamps the event on the calling thread, then hands it
   to a background writer queue. The writer adds the ids and session, encodes the event once and
   appends it to the open batch file.
2. A batch is closed when it reaches `batchSize` events or 512 KiB, every `flushInterval`, when the
   app goes to the background, and on `flush()`. Closed batches wait in an on-disk queue.
3. The uploader posts queued batches oldest first, one per request:
   - **Any 2xx:** the batch is delivered and deleted.
   - **400, 413, 422:** the collector will never accept the batch. It is dropped and the collector's error is logged.
   - **Anything else** (no network, timeouts, 5xx, 429, redirects, 401/403/404): the batch is kept. Uploads stop and retry with exponential backoff from 5 s up to 5 minutes, honouring `Retry-After` (up to an hour). Coming back to the foreground, or going to the background, retries at once.
4. Going to the background starts a background task, so the batch committed then can be uploaded;
   the task ends once the uploader has sent what it can or stopped at a failure, or when iOS runs
   out of time. Unsent batches wait on disk.

Delivery is **at least once**. A resend carries the same event ids and the same bytes (unless the
device's time zone changed in between), so the collector's downstream stores can deduplicate it.

| Limit | Value |
|---|---|
| Single event | 64 KiB encoded; larger events are dropped with a warning |
| Queue on disk | 10 MiB; the oldest batches are dropped beyond it |
| Age | Batches older than 7 days are dropped |
| Events waiting for the writer | 1,000 in memory; more are dropped with a warning |

Everything is stored under the app's `Library/Application Support/sudoquery/v1/`. Unlike Caches,
iOS never purges it; it is excluded from iCloud and Finder backups, so the anonymous id isn't
restored onto another device. Deleting the app starts over with a new anonymous id. The files use
the `completeUntilFirstUserAuthentication` protection class, so a background flush works while the
device is locked. Before the first unlock after a reboot (a background launch, e.g. for a VoIP push)
nothing can be read or written: events tracked then are dropped with a warning, and the SDK uses a
temporary identity that it never saves over the stored one. Once the device is unlocked it goes on
with the stored anonymous id and user, keeping any `setUser` or `reset` made meanwhile.

**Sessions:** a session continues while events keep coming, and a new one starts after
`sessionTimeout` without events. This holds across app restarts.

**App extensions** have their own container, so an extension that initializes the SDK has its own
queue and anonymous id. Extensions get no lifecycle hooks; their events go out on the timer and on
`flush()`.

## Wire format and changing the event type

All wire types live in one file, [`Sources/SudoQuery/Event.swift`](Sources/SudoQuery/Event.swift).
`EventV1` mirrors `CollectorEvent` in `crates/events-collector/src/collector_event.rs`, the TS
client's `Event` and the Android SDK's `EventV1`, including the `envelop_version` and `occured_at`
spellings, except that it has `workspace_id` where they have `project_id`. Every key is written out,
nulls included, in the same order:

```json
{"events":[{"envelop_version":"1.0","id":"3b241101-e2bb-4255-8caf-4136c566a962","name":"button_click",
"org_id":"acme-k3x9qa","workspace_id":"ios-demo1","session_id":"9f1c2d3e-…","anon_id":"6e8bc430-…",
"actor_id":null,"source":"ios","occured_at":"2026-10-14T09:30:12.345Z","properties":{"page":"/checkout"},
"correlation_id":null,"trace_id":null,"system_properties":null}],
"system_properties":{"geo":null,"timezone":"Asia/Kolkata"}}
```

The type the SDK sends is chosen by one line:

```swift
typealias CanonicalEvent = EventV1
```

To switch envelopes:
1. Add the new type to `Event.swift`, conforming to `WireEvent`: an `init(_ input: EventInput)` that fills it from what `track()` captured, and `json()` with its keys in wire order.
2. Point the alias at it.

Batching, storage and upload only handle encoded events, so nothing else changes. Events already
queued by an older app version are sent exactly as they were written. If the new type needs data
`EventInput` doesn't carry, add it to `EventInput` and to where `EventWriter` fills it.

## Differences from the TypeScript and Android clients

- **Like Android:** `endpoint` is required; events are persisted and retried by status code; the user id persists across restarts and `reset()` exists; sessions end after inactivity; calls before `initialize` are dropped with a warning instead of throwing; properties are always a JSON object.
- **The workspace id** goes out as `x-workspace-id` and `workspace_id`. The TS and Android clients call it `projectId` and send `x-project-id` and `project_id`.
- **`source` defaults to `"ios"`.**
- **Intervals are seconds**, as `TimeInterval`s.
- **Property keys are sorted** when a Swift or Objective-C dictionary (which has no order) is turned into JSON; super properties keep the order they were set in.
- **`Date`, `URL` and `UUID` values** are sent as ISO 8601, absolute and lowercase strings; Android sends `toString()`.
- **Backgrounding** runs the upload inside a background task; Android has no such limit to work around.
- **There is no process check:** on iOS an app's extensions already have separate containers.

## Privacy

The package ships a privacy manifest
([`Sources/SudoQuery/Resources/PrivacyInfo.xcprivacy`](Sources/SudoQuery/Resources/PrivacyInfo.xcprivacy)):

- **No tracking, no tracking domains, and no required-reason APIs.** The SDK uses neither UserDefaults nor file-timestamp, system-boot-time or disk-space APIs.
- **Collected data, all used for analytics and linked to the user:** product interaction (the events you track), a device id (the random `anon_id`), a user id (`actor_id`, after `setUser`), and coarse location (the collector derives the country from each request's IP address).

For App Store Connect's privacy labels: the SDK stores a random anonymous id and, after
`setUser`, your user id; it sends both with every event, with the properties you track and the
device's time zone. Nothing else is collected automatically.

## Plain HTTP

App Transport Security blocks `http://` endpoints. For a collector on your Mac during development,
add `NSAllowsLocalNetworking` under `NSAppTransportSecurity` in the app's Info.plist (the sample app
does); the iOS simulator reaches the Mac at `localhost`. Use HTTPS everywhere else.

## Development

From `clients/ios`:

```sh
swift build -Xswiftc -warnings-as-errors
swift test                                   # unit tests on the Mac (UIKit-only tests are skipped there)
xcodebuild test -scheme SudoQuery -destination 'platform=iOS Simulator,name=<an iPhone simulator>'
```

The `SudoQuery` package scheme runs the whole suite on the simulator, including the app-lifecycle
tests. The tests use a loopback HTTP server, so they need no network.

To run against a local collector from the repository root:

```sh
cargo xtask setup redpanda
docker compose exec redpanda rpk topic create events.acme-k3x9qa.ios-demo1
COLLECTOR_CONFIG=crates/events-collector/cac.toml SERVER_ADDR=0.0.0.0:3001 cargo run -p event-collector
```

- **Through the real pipeline:**
  ```sh
  (cd clients/ios && SUDOQUERY_LIVE_COLLECTOR_URL=http://localhost:3001/v1/events/batch swift test --filter LiveCollectorTests)
  ```
  For the simulator, run the `xcodebuild test` above with
  `TEST_RUNNER_SUDOQUERY_LIVE_COLLECTOR_URL=http://localhost:3001/v1/events/batch` and
  `-only-testing:SudoQueryTests/LiveCollectorTests`.
- **The sample app** (`Sample/SudoQuerySample.xcodeproj`, iOS 16+) talks to
  `http://localhost:3001/v1/events/batch` as org `acme-k3x9qa`, workspace `ios-demo1`; change the
  `SUDOQUERY_*` build settings or set them as environment variables. Its buttons mirror the Android
  sample, and launch arguments drive it from the command line:
  ```sh
  xcrun simctl launch <device> io.juspay.sudoquery.sample --sudoquery-smoke-test       # 3 events, flushed
  xcrun simctl launch <device> io.juspay.sudoquery.sample --sudoquery-objc-smoke       # the Objective-C API
  xcrun simctl launch <device> io.juspay.sudoquery.sample --sudoquery-background-test  # sent once backgrounded
  ```
- **Read what arrived:**
  ```sh
  docker compose exec redpanda rpk topic consume events.acme-k3x9qa.ios-demo1 -n 10
  ```
- **Logs:** `xcrun simctl spawn <device> log stream --predicate 'subsystem == "io.juspay.sudoquery"'`.

CI ([`.github/workflows/ios-client.yml`](../../.github/workflows/ios-client.yml)) runs the tests on
macOS and on an iOS simulator, checks that the SDK has no trapping calls (`try!`, `as!`,
`fatalError`, `precondition`) and references no Swift Concurrency runtime symbols, and builds the
sample, whose `ObjCSmokeTest.m` compile-checks the Objective-C header. iOS 13 to 15 are
compile-checked only; tests run on the simulator runtimes the CI image has.

## Publishing

Not set up yet. SwiftPM resolves a package by its git URL and reads `Package.swift` at the root of
that repository, at semver tags. Here the manifest is in `clients/ios`, and the repository's `v*`
tags belong to the npm package (`.github/workflows/release.yml` publishes on them), so publishing
needs either:

- a mirror repository (e.g. `juspay/sudoquery-ios`) that a release workflow fills from
  `git subtree split --prefix=clients/ios` and tags `X.Y.Z`, on `ios-v*` tags here; or
- a root `Package.swift`, which would also need tags that don't collide with npm's.

Either way the package needs a LICENSE file (the npm package and the Android SDK use ISC), and the
version in [`Sources/SudoQuery/Version.swift`](Sources/SudoQuery/Version.swift) is the one to tag.
