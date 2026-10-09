var ShopifySudoQueryPixel = (() => {
  // src/Configuration.ts
  var _Configuration = class _Configuration {
    static get batchSize() {
      return _Configuration._batchSize;
    }
    static get flushInterval() {
      return _Configuration._flushInterval;
    }
    static get endpoint() {
      return _Configuration._endpoint;
    }
    static get token() {
      return _Configuration._token;
    }
    static get headers() {
      return _Configuration._headers;
    }
    static get orgId() {
      return _Configuration._orgId;
    }
    static get projectId() {
      return _Configuration._projectId;
    }
    static get source() {
      return _Configuration._source;
    }
    static get sessionId() {
      return _Configuration._sessionId;
    }
    static setBatchSize(value) {
      _Configuration._batchSize = value;
    }
    static setFlushInterval(value) {
      _Configuration._flushInterval = value;
    }
    static setEndpoint(value) {
      _Configuration._endpoint = value;
    }
    static setToken(value) {
      _Configuration._token = value;
    }
    static setHeaders(value) {
      _Configuration._headers = value;
    }
    static setOrgId(value) {
      _Configuration._orgId = value;
    }
    static setProjectId(value) {
      _Configuration._projectId = value;
    }
    static setSource(value) {
      _Configuration._source = value;
    }
    static setSessionId(value) {
      _Configuration._sessionId = value;
    }
    static reset() {
      _Configuration._batchSize = 10;
      _Configuration._flushInterval = null;
      _Configuration._endpoint = _Configuration.DEFAULT_ENDPOINT;
      _Configuration._token = null;
      _Configuration._headers = {};
      _Configuration._orgId = null;
      _Configuration._projectId = null;
      _Configuration._source = _Configuration.DEFAULT_SOURCE;
      _Configuration._sessionId = null;
    }
  };
  _Configuration.DEFAULT_ENDPOINT = "http://localhost:3000/batch";
  _Configuration.DEFAULT_SOURCE = "typescript";
  _Configuration._batchSize = 10;
  _Configuration._flushInterval = null;
  _Configuration._endpoint = _Configuration.DEFAULT_ENDPOINT;
  _Configuration._token = null;
  _Configuration._headers = {};
  _Configuration._orgId = null;
  _Configuration._projectId = null;
  _Configuration._source = _Configuration.DEFAULT_SOURCE;
  _Configuration._sessionId = null;
  var Configuration = _Configuration;

  // src/Uuid.ts
  function generateUuid() {
    const runtimeCrypto = getRuntimeCrypto();
    if (runtimeCrypto && typeof runtimeCrypto.randomUUID === "function") {
      return runtimeCrypto.randomUUID();
    }
    const bytes = new Uint8Array(16);
    if (runtimeCrypto && typeof runtimeCrypto.getRandomValues === "function") {
      runtimeCrypto.getRandomValues(bytes);
    } else {
      for (let i = 0; i < bytes.length; i++) {
        bytes[i] = Math.floor(Math.random() * 256);
      }
    }
    bytes[6] = bytes[6] & 15 | 64;
    bytes[8] = bytes[8] & 63 | 128;
    const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0"));
    return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10, 16).join("")}`;
  }
  function getRuntimeCrypto() {
    return typeof globalThis !== "undefined" && "crypto" in globalThis ? globalThis.crypto : null;
  }

  // src/Session.ts
  var currentSessionId = null;
  function getSessionId() {
    if (!currentSessionId) {
      currentSessionId = generateId();
    }
    return currentSessionId;
  }
  function getSystemProperties() {
    return {
      geo: null,
      timezone: getTimezone()
    };
  }
  function getTimezone() {
    if (typeof Intl === "undefined") {
      return null;
    }
    return Intl.DateTimeFormat().resolvedOptions().timeZone ?? null;
  }
  function generateId() {
    return generateUuid();
  }

  // src/Pusher.ts
  var Pusher = class {
    static get endpoint() {
      return Configuration.endpoint;
    }
    /**
     * Wrap queued collector events in the collector batch format.
     */
    static transformBatch(batch) {
      return {
        events: batch,
        system_properties: getSystemProperties()
      };
    }
    static async pushLogs(useBeacon = false) {
      if (this._isUploadInProgress) return null;
      this._isUploadInProgress = true;
      const batch = Batcher.fetchBatchToUpload();
      if (!batch) {
        this._isUploadInProgress = false;
        return null;
      }
      const payload = this.transformBatch(batch);
      if (!payload) {
        this._isUploadInProgress = false;
        return null;
      }
      if (useBeacon) {
        this.sendWithKeepalive(payload);
        this._isUploadInProgress = false;
        return null;
      } else {
        const success = await this.sendNormally(payload);
        this._isUploadInProgress = false;
        return success ? batch : null;
      }
    }
    static sendWithKeepalive(payload) {
      if (typeof fetch === "undefined") {
        return false;
      }
      const headers = this.buildHeaders(payload);
      if (!headers) {
        return false;
      }
      try {
        void fetch(this.endpoint, {
          method: "POST",
          headers,
          body: JSON.stringify(payload),
          keepalive: true
        });
        return true;
      } catch (error) {
        console.error("Keepalive fetch failed:", error);
        return false;
      }
    }
    static async sendNormally(payload) {
      const headers = this.buildHeaders(payload);
      if (!headers) {
        return false;
      }
      try {
        const response = await fetch(this.endpoint, {
          method: "POST",
          headers,
          body: JSON.stringify(payload)
        });
        return response.ok;
      } catch (error) {
        console.error("Fetch failed:", error);
        return false;
      }
    }
    static buildHeaders(payload) {
      const orgId = Configuration.orgId ?? payload.events[0]?.org_id ?? null;
      if (!orgId) {
        console.error("Cannot send analytics batch: orgId is required by the collector.");
        return null;
      }
      const projectId = Configuration.projectId ?? payload.events[0]?.project_id ?? null;
      if (!projectId) {
        console.error("Cannot send analytics batch: projectId is required by the collector.");
        return null;
      }
      const headers = {
        "Content-Type": "application/json",
        ...Configuration.headers,
        "x-org-id": orgId,
        "x-project-id": projectId
      };
      if (Configuration.token) {
        headers.Authorization = `Bearer ${Configuration.token}`;
      }
      return headers;
    }
    static async startScheduler(time) {
      setInterval(() => {
        flush();
      }, time);
    }
  };
  Pusher._isUploadInProgress = false;

  // src/Flush.ts
  async function flush(useBeacon = false) {
    while (true) {
      const res = await Pusher.pushLogs(useBeacon);
      if (res === null) {
        return;
      } else {
        Batcher.setMarkLastBatchUploaded();
      }
    }
  }

  // src/Batcher.ts
  var Batcher = class {
    static addToBatch(event) {
      if (this.batches[this.currentAccumilatingBatch].length === Configuration.batchSize) {
        this.addNewBatch();
      }
      const lastBatch = this.batches[this.batches.length - 1];
      lastBatch.push(event);
      if (lastBatch.length === Configuration.batchSize) {
        flush(false).catch((err) => console.error("Auto-flush error:", err));
      }
    }
    static fetchBatchToUpload() {
      const batchToUpload = this.batches[this._currentBatchToUpload];
      if (batchToUpload.length == 0) return null;
      if (this._currentBatchToUpload === this.currentAccumilatingBatch) {
        this.addNewBatch();
      }
      return batchToUpload;
    }
    static addNewBatch() {
      this.batches.push([]);
      this.currentAccumilatingBatch = this.batches.length - 1;
    }
    static setMarkLastBatchUploaded() {
      return this._currentBatchToUpload++;
    }
    /**
     * Reset all internal state. Useful for testing.
     */
    static reset() {
      this.batches = [[]];
      this._currentBatchToUpload = 0;
      this.currentAccumilatingBatch = 0;
    }
  };
  Batcher.batches = [[]];
  Batcher._currentBatchToUpload = 0;
  Batcher.currentAccumilatingBatch = 0;

  // src/SuperProperties.ts
  var SuperProperties = class {
    static addToSuperProperties(key, value) {
      this.properties[key] = value;
    }
    static getSuperProperties() {
      return { ...this.properties };
    }
    static clearSuperProperties() {
      this.properties = {};
    }
  };
  SuperProperties.properties = {};

  // src/AnonymousId.ts
  var STORAGE_KEY = "hyper_analytics_anon_id";
  var AnonymousId = class {
    static generateId() {
      return generateUuid();
    }
    /**
     * Checks if running in browser environment
     */
    static isBrowser() {
      return typeof window !== "undefined";
    }
    /**
     * Gets the current anonymous ID.
     * - Browser: retrieves from localStorage, creates if not exists
     * - Node.js: retrieves from in-memory storage, creates if not exists
     *
     * @returns The anonymous ID string
     */
    static getOrCreate() {
      if (this.isBrowser()) {
        const storedAnonId = this.getFromStorage();
        if (storedAnonId) return storedAnonId;
        const anonId = this.generateId();
        this.setInStorage(anonId);
        return anonId;
      } else {
        if (!this.inMemoryAnonId) {
          this.inMemoryAnonId = this.generateId();
        }
        return this.inMemoryAnonId;
      }
    }
    /**
     * Resets the anonymous ID.
     * - Browser: removes from localStorage
     * - Node.js: clears in-memory storage
     * The next call to getOrCreate() will generate a new ID.
     */
    static reset() {
      if (this.isBrowser()) {
        this.removeFromStorage();
      } else {
        this.inMemoryAnonId = null;
      }
    }
    /**
     * Gets the current anonymous ID without creating a new one if it doesn't exist.
     * - Browser: reads from localStorage
     * - Node.js: reads from in-memory storage
     *
     * @returns The anonymous ID string, or null if not set
     */
    static get() {
      if (this.isBrowser()) {
        return this.getFromStorage();
      } else {
        return this.inMemoryAnonId;
      }
    }
    static getFromStorage() {
      try {
        return localStorage.getItem(STORAGE_KEY);
      } catch (_) {
        return this.inMemoryAnonId;
      }
    }
    static setInStorage(anonId) {
      try {
        localStorage.setItem(STORAGE_KEY, anonId);
      } catch (_) {
        this.inMemoryAnonId = anonId;
      }
    }
    static removeFromStorage() {
      try {
        localStorage.removeItem(STORAGE_KEY);
      } catch (_) {
      }
      this.inMemoryAnonId = null;
    }
  };
  // In-memory storage for Node.js environment
  AnonymousId.inMemoryAnonId = null;

  // src/SudoQuery.ts
  var SudoQuery = class {
    static init(config) {
      if (this.didInit) return;
      this.didInit = true;
      if (config?.batchSize !== void 0) {
        Configuration.setBatchSize(config.batchSize);
      }
      if (config?.endpoint !== void 0) {
        Configuration.setEndpoint(config.endpoint);
      }
      if (config?.token !== void 0) {
        Configuration.setToken(config.token);
      }
      if (config?.headers !== void 0) {
        Configuration.setHeaders(config.headers);
      }
      if (config?.orgId !== void 0) {
        warnIfNotSlug("orgId", config.orgId);
        Configuration.setOrgId(config.orgId);
      }
      if (config?.projectId !== void 0) {
        warnIfNotSlug("projectId", config.projectId);
        Configuration.setProjectId(config.projectId);
      }
      if (config?.source !== void 0) {
        Configuration.setSource(config.source);
      }
      if (config?.sessionId !== void 0) {
        Configuration.setSessionId(config.sessionId);
      }
      if (config?.flushInterval !== void 0 && config.flushInterval > 0) {
        Configuration.setFlushInterval(config.flushInterval);
        this.startPeriodicFlush(config.flushInterval);
      }
      if (typeof document !== "undefined") {
        document.addEventListener("visibilitychange", () => {
          if (document.visibilityState === "hidden") {
            this.flush(true).catch((err) => console.error("Flush on pagehide error:", err));
          }
        });
      }
    }
    static startPeriodicFlush(intervalMs) {
      if (this.flushTimer !== null) return;
      this.flushTimer = setInterval(() => {
        this.flush(false).catch((err) => console.error("Periodic flush error:", err));
      }, intervalMs);
    }
    /**
     * Check if the SDK has been initialized
     */
    static get isInitialized() {
      return this.didInit;
    }
    /**
     * Get the current batch size
     */
    static get batchSize() {
      return Configuration.batchSize;
    }
    /**
     * Get the current endpoint URL
     */
    static get endpoint() {
      return Configuration.endpoint;
    }
    /**
     * Add a property to super properties (included in all events)
     */
    static setSuperProperty(key, value) {
      SuperProperties.addToSuperProperties(key, value);
    }
    /**
     * Get all current super properties
     */
    static getSuperProperties() {
      return SuperProperties.getSuperProperties();
    }
    /**
     * Clear all super properties
     */
    static clearSuperProperties() {
      SuperProperties.clearSuperProperties();
    }
    /**
     * Flush all pending events to the server
     * @param useBeacon - Use fetch keepalive for more reliable delivery during page unload
     */
    static async flush(useBeacon = false) {
      await flush(useBeacon);
    }
    /**
     * Set the user ID for all subsequent events
     * @param userId - The user identifier
     */
    static setUser(userId) {
      this.currentUser = userId;
    }
    /**
     * Remove the current user ID (resets to null)
     */
    static removeUser() {
      this.currentUser = null;
    }
    /**
     * Get the current user ID
     * @returns The current user ID or null if not set
     */
    static getUser() {
      return this.currentUser;
    }
    static track(eventName, properties) {
      const props = properties ?? {};
      const orgId = Configuration.orgId;
      if (!orgId || orgId.trim().length === 0) {
        throw new Error("orgId is required before tracking events");
      }
      const projectId = Configuration.projectId;
      if (!projectId || projectId.trim().length === 0) {
        throw new Error("projectId is required before tracking events");
      }
      const superProperties = SuperProperties.getSuperProperties();
      const mergedProperties = mergeProperties(props, superProperties);
      const event = {
        envelop_version: "1.0",
        id: generateUuid(),
        name: eventName.toString(),
        org_id: orgId,
        project_id: projectId,
        session_id: Configuration.sessionId ?? getSessionId(),
        anon_id: AnonymousId.getOrCreate(),
        actor_id: this.currentUser,
        source: Configuration.source,
        occured_at: (/* @__PURE__ */ new Date()).toISOString(),
        properties: mergedProperties,
        correlation_id: null,
        trace_id: null,
        system_properties: null
      };
      Batcher.addToBatch(event);
    }
  };
  SudoQuery.didInit = false;
  SudoQuery.currentUser = null;
  SudoQuery.flushTimer = null;
  function mergeProperties(properties, defaults) {
    if (isJsonRecord(properties)) {
      return {
        ...defaults,
        ...properties
      };
    }
    return properties;
  }
  var SLUG_PATTERN = /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/;
  function warnIfNotSlug(field, value) {
    if (value === null || SLUG_PATTERN.test(value)) {
      return;
    }
    console.warn(
      `SudoQuery: config option "${field}" value "${value}" does not match the expected server-generated slug shape /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/ (e.g. "acme-store-k3x9qa"); the collector may reject events carrying it.`
    );
  }
  function isJsonRecord(value) {
    return typeof value === "object" && value !== null && !Array.isArray(value);
  }

  // shopify/index.js
  try {
    let addIdentifier = function(identifiers, type, rawValue) {
      if (rawValue === void 0 || rawValue === null) return;
      const value = String(rawValue).trim();
      if (!value) return;
      if (!identifiers[type]) identifiers[type] = [];
      identifiers[type].push({ value });
    }, buildIdentifiers = function(event) {
      const identifiers = {};
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
      const initCustomer = typeof init !== "undefined" ? init?.data?.customer : void 0;
      if (initCustomer) {
        addIdentifier(identifiers, "email", initCustomer.email);
        addIdentifier(identifiers, "phone", initCustomer.phone);
        addIdentifier(identifiers, "shopify_customer_id", initCustomer.id);
      }
      return identifiers;
    }, buildProperties = function(event) {
      const props = {
        shopify_event_id: event.id,
        shopify_event_name: event.name,
        shopify_timestamp: event.timestamp,
        shopify_client_id: event.clientId
      };
      const checkout = event.data && event.data.checkout;
      if (checkout) {
        if (checkout.order?.id) props.order_id = String(checkout.order.id);
        if (checkout.totalPrice?.amount !== void 0) props.amount = checkout.totalPrice.amount;
        if (checkout.currencyCode) props.currency = checkout.currencyCode;
        if (Array.isArray(checkout.lineItems)) {
          props.items = checkout.lineItems.map((li) => ({
            sku: li.variant?.sku,
            product_id: li.variant?.product?.id,
            variant_id: li.variant?.id,
            qty: li.quantity,
            price: li.variant?.price?.amount
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
    }, buildContext = function(event) {
      const doc = event.context?.document;
      const nav = event.context?.navigator;
      return {
        source: "browser",
        page_url: doc?.location?.href,
        referrer: doc?.referrer,
        user_agent: nav?.userAgent,
        locale: nav?.language
      };
    }, send = function(event) {
      const collectorName = SHOPIFY_TO_COLLECTOR_EVENT_NAME[event.name];
      if (!collectorName) return;
      try {
        SudoQuery.setSuperProperty("identifiers", buildIdentifiers(event));
        SudoQuery.track(collectorName, {
          context: buildContext(event),
          properties: buildProperties(event)
        });
      } catch (_) {
      }
    };
    addIdentifier2 = addIdentifier, buildIdentifiers2 = buildIdentifiers, buildProperties2 = buildProperties, buildContext2 = buildContext, send2 = send;
    const pixelSettings = typeof settings !== "undefined" ? settings : {};
    const COLLECTOR_ENDPOINT = pixelSettings.collectorEndpoint || "https://73g8lnmf-3000.inc1.devtunnels.ms/batch";
    const ORG_ID = pixelSettings.orgId || "breeze";
    const PROJECT_ID = pixelSettings.projectId || "d2cmerino";
    SudoQuery.init({
      endpoint: COLLECTOR_ENDPOINT,
      orgId: ORG_ID,
      projectId: PROJECT_ID,
      source: "shopify",
      batchSize: 1
    });
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
      checkout_completed: "order_completed"
    };
    Object.keys(SHOPIFY_TO_COLLECTOR_EVENT_NAME).forEach((shopifyEventName) => {
      analytics.subscribe(shopifyEventName, send);
    });
  } catch (_) {
  }
  var addIdentifier2;
  var buildIdentifiers2;
  var buildProperties2;
  var buildContext2;
  var send2;
})();
