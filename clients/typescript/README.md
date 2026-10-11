# SudoQuery

A lightweight TypeScript analytics SDK for tracking events in browser and Node.js applications. Collect user behavior data with automatic batching, anonymous user tracking, and reliable delivery.

## Features

- **Event Tracking** - Track custom events with properties
- **Automatic Batching** - Accumulate events locally to reduce network requests
- **Anonymous Users** - Track unauthenticated users with persistent anonymous IDs
- **Super Properties** - Attach default properties to all events
- **Auto-Flush** - Automatically sends events on page unload using `fetch` keepalive
- **Periodic Auto-Flush** - Configurable interval-based automatic event flushing
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
  orgId: 'acme-store-k3x9qa',
  projectId: 'acme-store-web-9t2r4m'
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
  orgId: 'acme-store-k3x9qa',    // Required: org id (server-generated slug)
  projectId: 'acme-store-web-9t2r4m', // Required: project id (server-generated slug)
  source: 'checkout-web',  // Optional source label (default: typescript)
  sessionId: 'session-1',  // Optional session id; generated if omitted
  token: 'YOUR_PROJECT_TOKEN',  // Optional bearer token for proxies/gateways
  headers: {              // Custom headers to send with requests (optional)
    'X-Api-Key': 'your-api-key',
    'X-Custom-Header': 'custom-value'
  }
});
```

**Important:** `orgId` and `projectId` must be set before tracking events because the collector requires the `org_id` / `project_id` event fields and the `x-org-id` / `x-project-id` request headers.

This sets up:
- A page visibility listener to automatically flush events when the user navigates away
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

Track events with custom properties. Properties must be JSON serializable:

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

Attach default properties to every event:

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

## Configuration Options

All configuration is done through the `init()` method:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `flushInterval` | `number \| undefined` | `undefined` | Interval in milliseconds for periodic auto-flush. If not set, periodic flush is disabled. |
| `batchSize` | `number \| undefined` | `10` | Number of events to accumulate before auto-flushing. |
| `endpoint` | `string \| undefined` | `"http://localhost:3000/batch"` | URL where events are sent. |
| `orgId` | `string \| null \| undefined` | `undefined` | Organization id (server-generated slug). Required before `track()`; sent as the event's `org_id` and the `x-org-id` header. |
| `projectId` | `string \| null \| undefined` | `undefined` | Project id (server-generated slug). Required before `track()`; sent as the event's `project_id` and the `x-project-id` header. |
| `source` | `string \| null \| undefined` | `"typescript"` | Optional source value written to each event. |
| `sessionId` | `string \| null \| undefined` | generated | Optional session id written to each event. |
| `token` | `string \| undefined` | `undefined` | Optional bearer token for proxies/gateways. |
| `headers` | `Record<string, string> \| undefined` | `{}` | Custom headers to include in all requests to the endpoint. |

### Example Configurations

**Default configuration:**
```typescript
SudoQuery.init({ orgId: 'acme-store-k3x9qa', projectId: 'acme-store-web-9t2r4m' });
// Uses: batchSize=10, endpoint="http://localhost:3000/batch", source="typescript", no periodic flush
```

**High-frequency tracking:**
```typescript
SudoQuery.init({
  orgId: 'acme-store-k3x9qa',
  projectId: 'acme-store-web-9t2r4m',
  flushInterval: 2000,   // Flush every 2 seconds
  batchSize: 50,         // Larger batches
  endpoint: 'https://analytics.example.com/batch'
});
```

**Low-latency mode:**
```typescript
SudoQuery.init({
  orgId: 'acme-store-k3x9qa',
  projectId: 'acme-store-web-9t2r4m',
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
| `track(eventName: string, properties: JSONSerializable)` | Track an event |
| `setSuperProperty(key: string, value: JSONSerializable)` | Add a super property |
| `getSuperProperties(): Record<string, JSONSerializable>` | Get all super properties |
| `clearSuperProperties()` | Remove all super properties |
| `flush(useBeacon?: boolean): Promise<void>` | Manually flush pending events |
| `get batchSize(): number` | Get current batch size (read-only) |
| `get endpoint(): string` | Get current endpoint URL (read-only) |

## Collector Payload

```typescript
type Event = {
  envelop_version: '1.0';
  id: string;
  name: string;
  org_id: string;
  project_id: string;
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

```typescript
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
