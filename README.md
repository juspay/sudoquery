# SudoQuery

A lightweight TypeScript analytics SDK for tracking events in browser and Node.js applications. Collect user behavior data with automatic batching, anonymous user tracking, and reliable delivery.

## Features

- **Event Tracking** - Track custom events with properties
- **Automatic Batching** - Accumulate events locally to reduce network requests
- **Anonymous Users** - Track unauthenticated users with anonymous IDs (persisted in `localStorage` in browsers)
- **Super Properties** - Attach default properties to every event
- **Auto-Flush** - Sends all pending events with `fetch` keepalive when the page is hidden
- **Periodic Auto-Flush** - Configurable interval-based automatic event flushing
- **Retry with Backoff** - Failed uploads are retried with exponential backoff and jitter
- **Cross-Platform** - Works in both browsers and Node.js
- **TypeScript Support** - Full type definitions included

## Installation

```bash
npm i sudo-query
```

## Quick Start

```typescript
import { SudoQuery } from 'sudo-query';

// Initialize the SDK
SudoQuery.init({
  tenantId: 'tenant-1',
  endpoint: 'https://collector.example.com/batch'
});

// Track an event
SudoQuery.track('button_click', {
  button_id: 'submit',
  page: '/checkout'
});
```

## Usage

### Initialization

Call `init()` once when your application loads. You can optionally configure:

```typescript
import { SudoQuery } from 'sudo-query';

SudoQuery.init({
  flushInterval: 5000,    // Auto-flush every 5 seconds (optional)
  batchSize: 20,          // Batch 20 events before flushing (default: 10)
  endpoint: 'https://api.example.com/batch',  // Custom endpoint (default: http://localhost:3000/batch)
  tenantId: 'tenant-1',    // Required by the collector
  workspaceId: 'workspace-1', // Optional collector workspace
  source: 'checkout-web',  // Optional source label (default: typescript)
  sessionId: 'session-1',  // Optional session id; generated if omitted
  token: 'YOUR_PROJECT_TOKEN',  // Optional bearer token for proxies/gateways
  headers: {              // Custom headers to send with requests (optional)
    'X-Api-Key': 'your-api-key',
    'X-Custom-Header': 'custom-value'
  },
  retryBaseDelay: 1000,   // First retry delay after a failed upload, doubling each time (default: 1000)
  retryMaxDelay: 60000    // Cap on the retry delay (default: 60000)
});
```

**Important:**
- `tenantId` must be set before tracking events because the collector requires both a `tenant_id` event field and an `x-tenant-id` request header. `track()` throws an error if it is missing.
- The default `endpoint` is `http://localhost:3000/batch`, intended for local development. Always set `endpoint` in production.
- Only the first `init()` call takes effect; later calls are ignored.

This sets up:
- A page visibility listener that sends all pending events when the page is hidden (the user switches tabs, navigates away, or closes the page)
- A periodic flush timer (if `flushInterval` is provided)

### Identifying Users

Set a user ID to associate events with authenticated users:

```typescript
// Set user ID
SudoQuery.setUser('user_123');

// Get current user ID
const userId = SudoQuery.getUser();

// Clear user ID (on logout)
SudoQuery.removeUser();
```

### Tracking Events

Track events with custom properties. Properties are optional and must be JSON serializable:

```typescript
SudoQuery.track('page_view', {
  page: '/home',
  referrer: 'https://google.com'
});

SudoQuery.track('purchase', {
  product_id: 'prod_456',
  price: 29.99,
  quantity: 2,
  metadata: {
    coupon: 'SUMMER'
  }
});
```

### Super Properties

Attach default properties to every event. Super properties are merged into the event's properties when those properties are an object (or omitted); if an event sets the same key, the event's value wins. Events whose properties are an array or a primitive value are sent without super properties.

```typescript
// Add super properties
SudoQuery.setSuperProperty('app_version', '1.0.0');
SudoQuery.setSuperProperty('environment', 'production');

// Get all super properties
const props = SudoQuery.getSuperProperties();

// Clear all super properties
SudoQuery.clearSuperProperties();
```

### Manual Flush

Force upload of pending events:

```typescript
await SudoQuery.flush();
```

`flush()` resolves without sending if an upload is already in progress or the SDK is waiting to retry after a failure (see [Delivery](#delivery)).

### Delivery

- **Batching** - Events are queued in memory and uploaded when a batch reaches `batchSize`, on the `flushInterval` timer, on `flush()`, or when the page is hidden. One upload is in flight at a time.
- **Retries** - If an upload fails (network error or non-2xx response), the batch stays queued and is retried after `retryBaseDelay`, doubling on each consecutive failure up to `retryMaxDelay`, with jitter. Uploads are paused while waiting; a successful upload resets the delay.
- **Page hide/unload** - When the page is hidden, every pending batch is sent with `fetch` keepalive, even during a retry wait. If a send fails and the page is still open, the batch is queued again.
- **Duplicates** - Each event has a unique `id`; collectors can deduplicate on it.
- **Limitations** - The queue is held in memory only, so events not yet sent are lost if the page is reloaded or the process exits. The queue has no size limit. Browsers cap keepalive request bodies at about 64 KB in total, so a very large backlog may not all be delivered on unload.

### Anonymous IDs and Sessions

- **Anonymous ID** - Generated on first use. In browsers it is stored in `localStorage` (Safari may clear it after 7 days without a visit); in Node.js it is kept in memory for the life of the process.
- **Session ID** - If `sessionId` is not configured, one is generated per page load (browser) or per process (Node.js). Pass `sessionId` to keep a session across page loads.

## Configuration Options

All configuration is done through the `init()` method:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `flushInterval` | `number \| undefined` | `undefined` | Interval in milliseconds for periodic auto-flush. If not set, periodic flush is disabled. |
| `batchSize` | `number \| undefined` | `10` | Number of events to accumulate before auto-flushing. |
| `endpoint` | `string \| undefined` | `"http://localhost:3000/batch"` | URL where events are sent. Set this in production. |
| `tenantId` | `string \| null \| undefined` | `undefined` | Collector tenant id. Required before tracking events. |
| `workspaceId` | `string \| null \| undefined` | `undefined` | Optional collector workspace id. Sent as `x-workspace-id` when set. |
| `source` | `string \| null \| undefined` | `"typescript"` | Optional source value written to each event. |
| `sessionId` | `string \| null \| undefined` | generated | Optional session id written to each event. Generated per page load (browser) or per process (Node.js) if omitted. |
| `token` | `string \| undefined` | `undefined` | Optional bearer token for proxies/gateways. |
| `headers` | `Record<string, string> \| undefined` | `{}` | Custom headers to include in all requests to the endpoint. |
| `retryBaseDelay` | `number \| undefined` | `1000` | Delay in milliseconds before the first retry after a failed upload. Doubles on each consecutive failure (with jitter). Values ≤ 0 are ignored. |
| `retryMaxDelay` | `number \| undefined` | `60000` | Maximum delay in milliseconds between retries. Values ≤ 0 are ignored. |

### Example Configurations

**Default configuration:**
```typescript
SudoQuery.init({ tenantId: 'tenant-1' });
// Uses: batchSize=10, endpoint="http://localhost:3000/batch", source="typescript", no periodic flush
```

**High-frequency tracking:**
```typescript
SudoQuery.init({
  tenantId: 'tenant-1',
  flushInterval: 2000,   // Flush every 2 seconds
  batchSize: 50,         // Larger batches
  endpoint: 'https://analytics.example.com/batch'
});
```

**Low-latency mode:**
```typescript
SudoQuery.init({
  tenantId: 'tenant-1',
  flushInterval: 1000,   // Flush every second
  batchSize: 5            // Small batches
});
```

## API Reference

### SudoQuery

All functionality is accessed through the `SudoQuery` class.

| Method | Description |
|--------|-------------|
| `init(config?)` | Initialize the SDK with optional configuration |
| `setUser(userId: string)` | Set current user ID |
| `getUser(): string \| null` | Get current user ID |
| `removeUser()` | Clear user ID |
| `track(eventName: string, properties?: JSONSerializable)` | Track an event. Throws if `tenantId` is not set |
| `setSuperProperty(key: string, value: JSONSerializable)` | Add a super property |
| `getSuperProperties(): Record<string, JSONSerializable>` | Get all super properties |
| `clearSuperProperties()` | Remove all super properties |
| `flush(useBeacon?: boolean): Promise<void>` | Manually flush pending events. `useBeacon: true` sends all pending batches with `fetch` keepalive (used on page hide) |
| `get batchSize(): number` | Get current batch size (read-only) |
| `get endpoint(): string` | Get current endpoint URL (read-only) |
| `get isInitialized(): boolean` | Whether `init()` has been called (read-only) |

## Collector Payload

```typescript
type Event = {
  envelop_version: '1.0';
  id: string;
  name: string;
  tenant_id: string;
  workspace_id: string | null;
  session_id: string | null;
  anon_id: string;
  actor_id: string | null;
  source: string | null;
  occured_at: string;
  properties: JSONSerializable | null;
  correlation_id: string | null;
  trace_id: string | null;
  system_properties: SystemProperties | null;
};

type BatchPayload = {
  events: Event[];
  system_properties: SystemProperties | null;
};

type SystemProperties = {
  geo: { country: string | null } | null;
  timezone: string | null;
};
```

## Types

The package exports these types: `SudoQueryConfig`, `JSONSerializable`, `Event`, `BatchPayload`, `SystemProperties`, `Geo`, and `EnvelopVersion`.

```typescript
import type { SudoQueryConfig, Event } from 'sudo-query';

type JSONSerializable =
  | string
  | number
  | boolean
  | null
  | JSONSerializable[]
  | { [key: string]: JSONSerializable };
```

## Browser Support

The SDK uses `localStorage` for persisting anonymous IDs and `fetch` keepalive for delivery during page unload. Works in all modern browsers.

## License

ISC
