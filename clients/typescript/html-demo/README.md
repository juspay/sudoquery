# SudoQuery HTML Demo

A small browser demo for the compiled SudoQuery bundle in `html-demo/dist/`.

## Features

- **User Management**: Set and remove the current actor id.
- **Event Tracking**: Track page view, button click, form submit, purchase, and error events.
- **Collector Payload**: Sends the collector batch shape to `http://localhost:3000/batch`.
- **Real-time Logging**: Shows tracked events in the page.
- **Auto-flush**: Flushes when the configured batch size is reached.

## How to Run

```bash
cd html-demo
node server.js 8000
```

Then open http://localhost:8000.

You can also use any static file server. Direct file opening may hit browser CORS limitations.

## Demo Configuration

The demo initializes the SDK with:

```javascript
SudoQuery.init({
  tenantId: 'localclient',
  batchSize: 3
});
```

The local collector requires `tenantId`; the SDK writes it into each event as `tenant_id` and sends it as the `x-tenant-id` header.

## Event Shape

Tracked events are queued using the collector envelope:

```javascript
{
  envelop_version: '1.0',
  id: 'uuid',
  name: 'event_name',
  tenant_id: 'localclient',
  workspace_id: null,
  session_id: 'session-id',
  anon_id: 'anonymous-id',
  actor_id: 'user_id',
  source: 'typescript',
  occured_at: '2026-08-25T12:00:00.000Z',
  properties: {},
  correlation_id: null,
  trace_id: null,
  system_properties: null
}
```

Batches are posted as:

```javascript
{
  events: [event],
  system_properties: {
    geo: null,
    timezone: 'Asia/Kolkata'
  }
}
```

## Browser Compatibility

The SDK uses `localStorage` for anonymous ids and `fetch` keepalive for page-unload delivery.
