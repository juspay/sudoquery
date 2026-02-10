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
npm install hyper-analytics
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
import { SuperProperties } from 'hyper-analytics';

// Add super properties
SuperProperties.addToSuperProperties('app_version', '1.0.0');
SuperProperties.addToSuperProperties('environment', 'production');

// Get all super properties
const props = SuperProperties.getSuperProperties();

// Clear all super properties
SuperProperties.clearSuperProperties();
```

### Manual Flush

Force upload of pending events:

```typescript
import { flush } from 'hyper-analytics';

await flush();
```

## Configuration

### Batch Size

Configure how many events to accumulate before auto-flushing:

```typescript
import { Configuration } from 'hyper-analytics';

Configuration.batchSize = 20; // Default is 10
```

### Server Endpoint

Set the analytics server endpoint:

```typescript
import { Pusher } from 'hyper-analytics';

Pusher.setEndpoint('https://api.yourcompany.com/analytics/batch');
// Default: http://localhost:3000/push_batch
```

### Periodic Auto-Flush

Start a scheduler to periodically flush events:

```typescript
import { Pusher } from 'hyper-analytics';

// Flush every 60 seconds
Pusher.startScheduler(60000);
```

## API Reference

### HyperAnalytics

| Method | Description |
|--------|-------------|
| `init()` | Initialize the SDK |
| `setUser(userId: string)` | Set current user ID |
| `getUser(): string \| null` | Get current user ID |
| `removeUser()` | Clear user ID |
| `track(eventName: string, properties: JSONSerializable)` | Track an event |

### Configuration

| Property | Type | Default |
|----------|------|---------|
| `batchSize` | number | 10 |

### SuperProperties

| Method | Description |
|--------|-------------|
| `addToSuperProperties(key: string, value: JSONSerializable)` | Add a super property |
| `getSuperProperties(): Record<string, JSONSerializable>` | Get all super properties |
| `clearSuperProperties()` | Remove all super properties |

### Pusher

| Method | Description |
|--------|-------------|
| `setEndpoint(url: string)` | Set server endpoint |
| `startScheduler(time: number)` | Start periodic flush timer |

### Utilities

| Function | Description |
|----------|-------------|
| `flush()` | Manually flush pending events |

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