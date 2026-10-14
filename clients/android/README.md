# SudoQuery Android SDK

The Android client for the SudoQuery events collector: the Kotlin counterpart of
[`clients/typescript`](../typescript). Track events from anywhere in your app; the SDK batches
them, keeps them on disk until they are delivered, and posts them to the collector's
`POST /v1/events/batch` route.

- **Durable:** events are written to disk as they are tracked and survive app restarts and crashes.
- **Batched:** one request per batch, sent when a batch fills up, on a timer, and when the app goes to the background.
- **Safe to call anywhere:** every method is thread-safe, returns at once and never throws. Disk and network work runs on background threads.
- **Small:** Kotlin, coroutines, kotlinx.serialization and `lifecycle-process`. No HTTP library is added to your app.

## Installation

```kotlin
dependencies {
    implementation("io.juspay:sudoquery-android:0.1.0")
}
```

Requirements: minSdk 21, compileSdk 34 or newer, and Kotlin 2.0 or newer (Java apps work too).
The SDK's manifest adds the `INTERNET` permission.

## Quick start

Initialize once, in `Application.onCreate`:

```kotlin
class App : Application() {
    override fun onCreate() {
        super.onCreate()
        SudoQuery.init(
            this,
            SudoQueryConfig.Builder(
                orgId = "acme-store-k3x9qa",
                projectId = "acme-store-app-9t2r4m",
                endpoint = "https://collector.example.com/v1/events/batch",
            ).build(),
        )
    }
}
```

Then track events from anywhere:

```kotlin
SudoQuery.track("button_click", mapOf("button_id" to "submit", "page" to "/checkout"))

SudoQuery.setUser("user_123")             // sent as actor_id from now on, and after restarts
SudoQuery.setSuperProperty("plan", "pro") // added to every event
SudoQuery.reset()                         // on logout: new anonymous id and session, no user
```

From Java the same methods are static:

```java
SudoQuery.init(this, new SudoQueryConfig.Builder(orgId, projectId, endpoint).build());
SudoQuery.track("button_click", Collections.singletonMap("button_id", "submit"));
```

`orgId` and `projectId` are the server-generated slugs shown in the dashboard's project settings
(the collector reads them from `x-org-id` / `x-project-id` and from every event).

## Configuration

`SudoQueryConfig.Builder(orgId, projectId, endpoint)` takes:

| Option | Default | Description |
|---|---|---|
| `endpoint` | required | Full URL of the batch route, e.g. `https://collector.example.com/v1/events/batch`. |
| `token(String?)` | none | Sent as `Authorization: Bearer <token>`, for a proxy or gateway in front of the collector. |
| `headers(Map)` | none | Extra headers. They can't replace `x-org-id`, `x-project-id` or the bearer token. |
| `source(String?)` | `"android"` | The `source` of every event. |
| `sessionId(String?)` | managed | Use a fixed session id instead of SDK-managed sessions. |
| `batchSize(Int)` | `50` | Events per batch and request (1 to 500). |
| `flushIntervalMillis(Long)` | `30000` | Send pending events at least this often; `0` turns the timer off. |
| `sessionTimeoutMillis(Long)` | 30 min | A new session starts after this long without events. |
| `debugLogging(Boolean)` | `false` | Debug logs under the `SudoQuery` logcat tag. Warnings and errors are always logged. |

`build()` throws `IllegalArgumentException` for a blank id, a non-http(s) endpoint or an
out-of-range value. Ids that don't look like slugs only log a warning.

## API

| Method | Description |
|---|---|
| `init(context, config)` | Starts the SDK. The first call wins; later calls are ignored. Does nothing outside the main process. |
| `track(name, properties)` | Records an event. Properties are a `Map<String, Any?>` or an `org.json.JSONObject`; nested maps, lists and arrays are supported. NaN and infinite numbers become `null`. |
| `setUser(id)` / `removeUser()` / `getUser()` | The user id sent as `actor_id`. It is kept across app restarts. |
| `setSuperProperty(key, value)` / `getSuperProperties()` / `clearSuperProperties()` | Properties added to every event (the event's own keys win). In memory only; usable before `init`. |
| `flush()` | Sends the events tracked so far now instead of waiting for a full batch. |
| `reset()` | For logout: later events get a new anonymous id and session, with no user id and no super properties. |
| `isInitialized` | True once `init` has started the SDK. |

Events tracked before `init` are dropped with a warning.

## How events are delivered

1. `track()` copies the properties and timestamps the event on the calling thread, then hands it
   to a background writer. The writer adds the ids and session, serializes the event once and
   appends it to the open batch file.
2. A batch is closed when it reaches `batchSize` events or 512 KiB, every `flushIntervalMillis`,
   when the app goes to the background, and on `flush()`. Closed batches wait in an on-disk queue.
3. The uploader posts queued batches oldest first, one per request:
   - **Any 2xx:** the batch is delivered and deleted.
   - **400, 413, 422:** the collector will never accept the batch. It is dropped and the collector's error is logged.
   - **Anything else** (no network, timeouts, 5xx, 429, redirects, 401/403/404): the batch is kept. Uploads stop and retry with exponential backoff from 5 s up to 5 minutes, honouring `Retry-After`. Coming back to the foreground retries at once.

Delivery is **at least once**. A resend is byte-for-byte the same request with the same event ids,
so the collector's downstream stores can deduplicate it.

| Limit | Value |
|---|---|
| Single event | 64 KiB serialized; larger events are dropped with a warning |
| Queue on disk | 10 MiB; the oldest batches are dropped beyond it |
| Age | Batches older than 7 days are dropped |
| Events waiting for the writer | 1,000 in memory; more are dropped with a warning |

Everything is stored under the app's `noBackupFilesDir/sudoquery/`. The OS never purges it, and
Auto Backup never restores it, so the anonymous id isn't copied to another device. Clearing app
data starts over with a new anonymous id. The SDK runs only in the app's main process.

**Sessions:** a session continues while events keep coming, and a new one starts after
`sessionTimeoutMillis` without events. This holds across app restarts.

## Wire format and changing the event type

All wire types live in one file,
[`sudoquery/src/main/kotlin/io/juspay/sudoquery/Event.kt`](sudoquery/src/main/kotlin/io/juspay/sudoquery/Event.kt).
`EventV1` mirrors `CollectorEvent` in `crates/events-collector/src/collector_event.rs` and the TS
client's `Event`, including the `envelop_version` and `occured_at` spellings. Every key is written
out, nulls included:

```json
{"events":[{"envelop_version":"1.0","id":"3b241101-e2bb-4255-8caf-4136c566a962","name":"button_click",
"org_id":"acme-k3x9qa","project_id":"android-demo1","session_id":"9f1c2d3e-…","anon_id":"6e8bc430-…",
"actor_id":null,"source":"android","occured_at":"2026-10-14T09:30:12.345Z","properties":{"page":"/checkout"},
"correlation_id":null,"trace_id":null,"system_properties":null}],
"system_properties":{"geo":null,"timezone":"Asia/Kolkata"}}
```

The class the SDK sends is chosen by one line:

```kotlin
internal typealias CanonicalEvent = EventV1
```

To switch envelopes:
1. Add the new `@Serializable` class to `Event.kt`, with a `constructor(input: EventInput)` that fills it from what `track()` captured.
2. Point the alias at it.

Batching, storage and upload only handle serialized events, so nothing else changes. Events already
queued by an older app version are sent exactly as they were written. If the new class needs data
`EventInput` doesn't carry, add it to `EventInput` and to where `EventWriter` fills it.

## Differences from the TypeScript client

- **`endpoint` is required:** there is no default. Point it at `/v1/events/batch`.
- **Events are persisted** and retried by status code, instead of held in memory and retried forever.
- **The user id persists across restarts**, and `reset()` is added for logout.
- **Sessions end after inactivity** instead of on every page load.
- **Calls before `init` are dropped** with a warning instead of throwing.
- **Properties are always a JSON object** (a map or `JSONObject`).
- **`source` defaults to `"android"`.**

## Privacy

For the Play Console's Data safety form:
- The SDK stores a random anonymous id (`anon_id`) and, after `setUser`, your user id (`actor_id`).
- It sends both with every event, together with the properties you track and the device's time zone.
- The collector derives the client IP address and country from each request.

Nothing else is collected automatically.

## Old devices

Android 7.0 (API 24) and older don't trust some newer certificate authorities, including Let's
Encrypt's root, so serve the collector with a certificate those devices accept if you support them.

## Development

The build needs JDK 17 and the Android SDK. AGP is kept at 9.1 so Android Studio Panda 3 can open the project.

```sh
cd clients/android
./gradlew check                        # unit tests and lint
./gradlew :sample:assembleRelease      # R8-minified sample app
./gradlew :sudoquery:publishToMavenLocal
```

To run against a local collector from the repository root:

```sh
cargo xtask setup redpanda
docker compose exec redpanda rpk topic create events.acme-k3x9qa.android-demo1
COLLECTOR_CONFIG=crates/events-collector/cac.toml SERVER_ADDR=0.0.0.0:3001 cargo run -p event-collector
```

- **Through the real pipeline, from the JVM:**
  ```sh
  (cd clients/android && ./gradlew :sudoquery:testDebugUnitTest --tests '*LiveCollectorTest*' \
      -Psudoquery.liveCollectorUrl=http://localhost:3001/v1/events/batch)
  ```
- **The sample app on an emulator** (10.0.2.2 is the host machine):
  ```sh
  (cd clients/android && ./gradlew :sample:installRelease -Psudoquery.endpoint=http://10.0.2.2:3001/v1/events/batch)
  ```
- **Read what arrived:**
  ```sh
  docker compose exec redpanda rpk topic consume events.acme-k3x9qa.android-demo1 -n 10
  ```

## Releasing

Releases go to Maven Central from `.github/workflows/release-android.yml`:
1. Set `VERSION_NAME` in `gradle.properties`.
2. Push the tag `android-v<VERSION_NAME>`.

The workflow checks the tag, runs the tests, uploads and validates the release. You then publish
it with the "Publish" button on [central.sonatype.com](https://central.sonatype.com).

One-time setup:
- Verify the `io.juspay` namespace on Central (a DNS TXT record on juspay.io).
- Create a Central user token.
- Create a GPG signing key and upload the public key to keys.openpgp.org.
- Add the repository secrets `MAVEN_CENTRAL_USERNAME`, `MAVEN_CENTRAL_PASSWORD`, `SIGNING_KEY` (the ASCII-armored private key) and `SIGNING_KEY_PASSWORD`.
