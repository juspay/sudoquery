import { SudoQuery } from "../src/index";

// Shopify "Customer events" custom pixel -> SudoQuery collector SDK.
//
// Build this source with `npm run build`, then paste shopify/dist/index.js into:
// Shopify Admin -> Settings -> Customer events -> Add custom pixel -> Edit code.
// Runs in Shopify's sandboxed pixel context,
// which exposes `analytics`, `browser`, `init`, `settings` as globals.

try {
  const pixelSettings = typeof settings !== "undefined" ? settings : {};
  const COLLECTOR_ENDPOINT =
    pixelSettings.collectorEndpoint || "https://73g8lnmf-3000.inc1.devtunnels.ms/batch";
  const TENANT_ID = pixelSettings.tenantId || "breeze";
  const WORKSPACE_ID = pixelSettings.workspaceId || "d2cmerino";

  SudoQuery.init({
    endpoint: COLLECTOR_ENDPOINT,
    tenantId: TENANT_ID,
    workspaceId: WORKSPACE_ID,
    source: "shopify",
    batchSize: 1,
  });

  // Shopify standard event name → collector event name. Identity mappings are
  // listed explicitly (rather than falling back to the Shopify name) so the
  // full set of events this pixel can send is visible at a glance.
  const SHOPIFY_TO_COLLECTOR_EVENT_NAME = {
    page_viewed: "page_viewed",
    collection_viewed: "collection_viewed",
    product_viewed: "product_viewed",
    search_submitted: "search_submitted",
    product_added_to_cart: "add_to_cart",
    product_removed_from_cart: "product_removed_from_cart",
    cart_viewed: "cart_viewed",
    checkout_started: "checkout_started",
    checkout_contact_info_submitted: "checkout_contact_info_submitted",
    checkout_address_info_submitted: "checkout_address_info_submitted",
    checkout_shipping_info_submitted: "checkout_shipping_info_submitted",
    payment_info_submitted: "payment_info_submitted",
    // Canonical name per the BreezeIQ doc / cdp-contracts test fixture,
    // deliberately not "checkout_completed" - this is what carries the
    // order_id dedup rule server-side.
    checkout_completed: "order_completed",
  };

  function addIdentifier(identifiers, type, rawValue) {
    if (rawValue === undefined || rawValue === null) return;
    const value = String(rawValue).trim();
    if (!value) return;
    if (!identifiers[type]) identifiers[type] = [];
    identifiers[type].push({ value });
  }

  function buildIdentifiers(event) {
    const identifiers = {};
    // Anonymous visitor id, persisted by Shopify across the browsing session —
    // present on every event, so this is what makes every event resolvable
    // even before any PII shows up.
    addIdentifier(identifiers, "cookie", event.clientId);

    const checkout = event.data && event.data.checkout;
    if (checkout) {
      addIdentifier(identifiers, "email", checkout.email);
      addIdentifier(
        identifiers,
        "phone",
        checkout.phone || checkout.shippingAddress?.phone || checkout.billingAddress?.phone
      );
      if (checkout.order?.customer?.id) {
        addIdentifier(identifiers, "shopify_customer_id", checkout.order.customer.id);
      }
    }

    // Logged-in customer, when Shopify populates it on init (storefront pages
    // where the buyer is already signed in).
    const initCustomer = typeof init !== "undefined" ? init?.data?.customer : undefined;
    if (initCustomer) {
      addIdentifier(identifiers, "email", initCustomer.email);
      addIdentifier(identifiers, "phone", initCustomer.phone);
      addIdentifier(identifiers, "shopify_customer_id", initCustomer.id);
    }

    return identifiers;
  }

  function buildProperties(event) {
    const props = {
      shopify_event_id: event.id,
      shopify_event_name: event.name,
      shopify_timestamp: event.timestamp,
      shopify_client_id: event.clientId,
      context: buildContext(event),
      identifiers: buildIdentifiers(event),
    };

    const checkout = event.data && event.data.checkout;
    if (checkout) {
      if (checkout.order?.id) props.order_id = String(checkout.order.id); // dedup key for order_completed
      if (checkout.totalPrice?.amount !== undefined) props.amount = checkout.totalPrice.amount;
      if (checkout.currencyCode) props.currency = checkout.currencyCode;
      if (Array.isArray(checkout.lineItems)) {
        props.items = checkout.lineItems.map((li) => ({
          sku: li.variant?.sku,
          product_id: li.variant?.product?.id,
          variant_id: li.variant?.id,
          qty: li.quantity,
          price: li.variant?.price?.amount,
        }));
      }
    }

    const productVariant = event.data && event.data.productVariant;
    if (productVariant) {
      props.product_id = productVariant.product?.id;
      props.variant_id = productVariant.id;
      props.sku = productVariant.sku;
      props.price = productVariant.price?.amount;
    }

    const cartLine = event.data && event.data.cartLine;
    if (cartLine) {
      props.product_id = cartLine.merchandise?.product?.id;
      props.variant_id = cartLine.merchandise?.id;
      props.qty = cartLine.quantity;
    }

    const searchResult = event.data && event.data.searchResult;
    if (searchResult?.query) props.query = searchResult.query;

    return props;
  }

  function buildContext(event) {
    const doc = event.context?.document;
    const nav = event.context?.navigator;
    return {
      source: "browser",
      page_url: doc?.location?.href,
      referrer: doc?.referrer,
      user_agent: nav?.userAgent,
      locale: nav?.language,
    };
  }

  function setActor(event) {
    const checkout = event.data && event.data.checkout;
    const initCustomer = typeof init !== "undefined" ? init?.data?.customer : undefined;
    const actorId =
      checkout?.order?.customer?.id ||
      initCustomer?.id ||
      checkout?.email ||
      initCustomer?.email ||
      null;

    if (actorId) {
      SudoQuery.setUser(String(actorId));
    } else {
      SudoQuery.removeUser();
    }
  }

  function send(event) {
    const collectorName = SHOPIFY_TO_COLLECTOR_EVENT_NAME[event.name];
    if (!collectorName) return; // not mapped — drop rather than send an unknown event name

    try {
      setActor(event);
      SudoQuery.track(collectorName, buildProperties(event));
    } catch (_) {
      // Best-effort analytics: never let a delivery failure surface in the storefront.
    }
  }

  Object.keys(SHOPIFY_TO_COLLECTOR_EVENT_NAME).forEach((shopifyEventName) => {
    analytics.subscribe(shopifyEventName, send);
  });
} catch (_) {
  // Never let analytics initialization failures surface in the storefront.
}
