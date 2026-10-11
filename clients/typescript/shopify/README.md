# Shopify Client

This folder contains the Shopify Customer Events custom pixel source for SudoQuery.

The source file is `shopify/index.js`. It is not the file to paste into Shopify because it imports the local SDK source. The build creates a single standalone browser bundle at `dist/shopify/index.js`.

## What It Does

The client runs inside Shopify's sandboxed customer events iframe and subscribes to Shopify standard analytics events through the `analytics` global.

It initializes SudoQuery with:

- `source: "shopify"`
- `batchSize: 1`
- `orgId` from pixel settings, defaulting to `breeze`
- `projectId` from pixel settings, defaulting to `d2cmerino`
- `collectorEndpoint` from pixel settings, defaulting to `https://73g8lnmf-3000.inc1.devtunnels.ms/batch`

It maps Shopify events into collector event names:

- `product_added_to_cart` -> `add_to_cart`
- `checkout_completed` -> `order_completed`
- Other subscribed Shopify events keep their Shopify event name.

The emitted collector event keeps `actor_id` as `null`. For the CDP use case, customer identity is carried in event properties instead:

```json
{
  "properties": {
    "identifiers": {
      "cookie": [{ "value": "shopify-client-id" }],
      "email": [{ "value": "buyer@example.com" }]
    },
    "context": {
      "source": "browser",
      "page_url": "https://store.myshopify.com/products/example",
      "referrer": "https://store.myshopify.com/",
      "user_agent": "Mozilla/5.0 ...",
      "locale": "en-US"
    },
    "properties": {
      "shopify_event_id": "sh-...",
      "shopify_event_name": "product_viewed",
      "shopify_timestamp": "2026-08-25T14:15:56.361Z",
      "shopify_client_id": "shopify-client-id",
      "product_id": "7455272075377",
      "variant_id": "42279731822705",
      "sku": null,
      "price": 1499
    }
  }
}
```

`identifiers` is set as a SudoQuery super property before each event. The rest of the Shopify event fields are tucked under the nested `properties` key so they cannot overwrite the reserved `identifiers` key.

The SDK still sends batch-level `system_properties`, including timezone from `Intl.DateTimeFormat().resolvedOptions().timeZone`.

## Build

Install dependencies if needed:

```bash
npm install
```

Build the package and Shopify client:

```bash
npm run build
```

Build only the Shopify client:

```bash
npm run build:shopify
```

The Shopify bundle is generated here:

```text
dist/shopify/index.js
```

The generated bundle is a single IIFE file. It includes the SudoQuery SDK code, has no ESM imports, has no CDN dependency, and the Shopify build strips the leading `"use strict";` directive.

## Install In Shopify

Copy the entire contents of:

```text
dist/shopify/index.js
```

Paste it into:

```text
Shopify Admin -> Settings -> Customer events -> Add custom pixel -> Edit code
```

Do not paste `shopify/index.js`; that is the source entry used by the bundler.

## Pixel Settings

When Shopify pixel settings are available, these keys override the built-in defaults:

```json
{
  "collectorEndpoint": "https://collector.example.com/batch",
  "orgId": "acme-org-1",
  "projectId": "acme-project-1"
}
```

The iframe runtime must allow outbound `fetch` calls to the collector endpoint, and the collector must allow requests from the Shopify pixel origin.
