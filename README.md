# Hyper-Analytics

A lightweight TypeScript analytics SDK for tracking events in browser and Node.js applications. Collect user behavior data with automatic batching, anonymous user tracking, and reliable delivery.

## Features

- **Event Tracking** - Track custom events with properties
- **Automatic Batching** - Accumulate events locally to reduce network requests
- **Anonymous Users** - Track unauthenticated users with persistent anonymous IDs
- **Super Properties** - Attach default properties to all events
- **Auto-Flush** - Automatically sends events on page unload using `navigator.sendBeacon()`
- **Periodic Auto-Flush** - Configurable interval-based automatic event flushing
- **Cross-Platform** - Works in both browsers and Node.js
- **TypeScript Support** - Full type definitions included

## Installation

```bash
npm install git+ssh://git@ssh.bitbucket.juspay.net/~sridatta.yalla_juspay.in/hyper-analytics-ts.git#main
```

## Quick Start

```typescript
import { HyperAnalytics } from 'hyper-analytics';

// Initialize the SDK
HyperAnalytics.init();

// Track an event
HyperAnalytics.track('button_click', {
  button_id: 'submit',
  page: '/checkout'
});
```

## Usage

### Initialization

Call `init()` once when your application loads. You can optionally configure:

```typescript
import { HyperAnalytics } from 'hyper-analytics';

HyperAnalytics.init({
  flushInterval: 5000,    // Auto-flush every 5 seconds (optional)
  batchSize: 20,          // Batch 20 events before flushing (default: 10)
  endpoint: 'https://api.example.com/events',  // Custom endpoint (default: http://localhost:3000/push_batch)
  token: 'YOUR_PROJECT_TOKEN'  // Project token for authentication (required)
});
```

**Important:** Configuration can only be set during initialization and cannot be modified afterward.

This sets up:
- A page visibility listener to automatically flush events when the user navigates away
- A periodic flush timer (if `flushInterval` is provided)

### Identifying Users

Set a user ID to associate events with authenticated users:

```typescript
// Set user ID
HyperAnalytics.setUser('user_123');

// Get current user ID
const userId = HyperAnalytics.getUser();

// Clear user ID (on logout)
HyperAnalytics.removeUser();
```

### Tracking Events

Track events with custom properties. Properties must be JSON serializable (primitives only - no nested objects):

```typescript
HyperAnalytics.track('page_view', {
  page: '/home',
  referrer: 'https://google.com'
});

HyperAnalytics.track('purchase', {
  product_id: 'prod_456',
  price: 29.99,
  quantity: 2
});
```

### Super Properties

Attach default properties to every event:

```typescript
// Add super properties
HyperAnalytics.setSuperProperty('app_version', '1.0.0');
HyperAnalytics.setSuperProperty('environment', 'production');

// Get all super properties
const props = HyperAnalytics.getSuperProperties();

// Clear all super properties
HyperAnalytics.clearSuperProperties();
```

### Manual Flush

Force upload of pending events:

```typescript
await HyperAnalytics.flush();
```

## Configuration Options

All configuration is done through the `init()` method:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `flushInterval` | `number \| undefined` | `undefined` | Interval in milliseconds for periodic auto-flush. If not set, periodic flush is disabled. |
| `batchSize` | `number \| undefined` | `10` | Number of events to accumulate before auto-flushing. |
| `endpoint` | `string \| undefined` | `"http://localhost:3000/push_batch"` | URL where events are sent. |
| `token` | `string \| undefined` | `undefined` | Project token for authentication. Required for sending events. |

### Example Configurations

**Default configuration:**
```typescript
HyperAnalytics.init();
// Uses: batchSize=10, endpoint="http://localhost:3000/push_batch", no periodic flush
```

**High-frequency tracking:**
```typescript
HyperAnalytics.init({
  flushInterval: 2000,   // Flush every 2 seconds
  batchSize: 50,         // Larger batches
  endpoint: 'https://analytics-api.example.com/batch'
});
```

**Low-latency mode:**
```typescript
HyperAnalytics.init({
  flushInterval: 1000,   // Flush every second
  batchSize: 5            // Small batches
});
```

## API Reference

### HyperAnalytics

All functionality is accessed through the `HyperAnalytics` class.

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

The SDK uses `localStorage` for persisting anonymous IDs and `navigator.sendBeacon()` for reliable delivery during page unload. Works in all modern browsers.

## License

ISC