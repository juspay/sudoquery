# Hyper-Analytics

A lightweight TypeScript analytics SDK for tracking events in browser and Node.js applications. Collect user behavior data with automatic batching, anonymous user tracking, and reliable delivery.

## Features

- **Event Tracking** - Track custom events with properties
- **Automatic Batching** - Accumulate events locally to reduce network requests
- **Anonymous Users** - Track unauthenticated users with persistent anonymous IDs
- **Super Properties** - Attach default properties to all events
- **Auto-Flush** - Automatically sends events on page unload using `navigator.sendBeacon()`
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

Call `init()` once when your application loads:

```typescript
import { HyperAnalytics } from 'hyper-analytics';

HyperAnalytics.init();
```

This sets up a page visibility listener to automatically flush events when the user navigates away.

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

## Configuration

### Batch Size

Configure how many events to accumulate before auto-flushing:

```typescript
HyperAnalytics.batchSize = 20; // Default is 10
```

### Server Endpoint

Set the analytics server endpoint:

```typescript
HyperAnalytics.setEndpoint('https://api.yourcompany.com/analytics/batch');
// Default: http://localhost:3000/push_batch
```

### Periodic Auto-Flush

Start a scheduler to periodically flush events:

```typescript
// Flush every 60 seconds
HyperAnalytics.startScheduler(60000);
```

## API Reference

### HyperAnalytics

All functionality is accessed through the `HyperAnalytics` class.

| Method | Description |
|--------|-------------|
| `init()` | Initialize the SDK |
| `setUser(userId: string)` | Set current user ID |
| `getUser(): string \| null` | Get current user ID |
| `removeUser()` | Clear user ID |
| `setGroup(groupId: string)` | Set current group ID |
| `getGroup(): string \| null` | Get current group ID |
| `removeGroup()` | Clear group ID |
| `track(eventName: string, properties: JSONSerializable)` | Track an event |
| `setSuperProperty(key: string, value: JSONSerializable)` | Add a super property |
| `getSuperProperties(): Record<string, JSONSerializable>` | Get all super properties |
| `clearSuperProperties()` | Remove all super properties |
| `flush(useBeacon?: boolean): Promise<void>` | Manually flush pending events |
| `set batchSize(value: number)` | Set batch size |
| `get batchSize(): number` | Get current batch size |
| `setEndpoint(url: string)` | Set server endpoint |
| `startScheduler(intervalMs: number)` | Start periodic flush timer |

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
