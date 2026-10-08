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
  static get tenantId() {
    return _Configuration._tenantId;
  }
  static get workspaceId() {
    return _Configuration._workspaceId;
  }
  static get source() {
    return _Configuration._source;
  }
  static get sessionId() {
    return _Configuration._sessionId;
  }
  static get retryBaseDelay() {
    return _Configuration._retryBaseDelay;
  }
  static get retryMaxDelay() {
    return _Configuration._retryMaxDelay;
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
  static setTenantId(value) {
    _Configuration._tenantId = value;
  }
  static setWorkspaceId(value) {
    _Configuration._workspaceId = value;
  }
  static setSource(value) {
    _Configuration._source = value;
  }
  static setSessionId(value) {
    _Configuration._sessionId = value;
  }
  static setRetryBaseDelay(value) {
    _Configuration._retryBaseDelay = value;
  }
  static setRetryMaxDelay(value) {
    _Configuration._retryMaxDelay = value;
  }
  static reset() {
    _Configuration._batchSize = 10;
    _Configuration._flushInterval = null;
    _Configuration._endpoint = _Configuration.DEFAULT_ENDPOINT;
    _Configuration._token = null;
    _Configuration._headers = {};
    _Configuration._tenantId = null;
    _Configuration._workspaceId = null;
    _Configuration._source = _Configuration.DEFAULT_SOURCE;
    _Configuration._sessionId = null;
    _Configuration._retryBaseDelay = _Configuration.DEFAULT_RETRY_BASE_DELAY;
    _Configuration._retryMaxDelay = _Configuration.DEFAULT_RETRY_MAX_DELAY;
  }
};
_Configuration.DEFAULT_ENDPOINT = "http://localhost:3000/batch";
_Configuration.DEFAULT_SOURCE = "typescript";
_Configuration.DEFAULT_RETRY_BASE_DELAY = 1e3;
_Configuration.DEFAULT_RETRY_MAX_DELAY = 6e4;
_Configuration._batchSize = 10;
_Configuration._flushInterval = null;
_Configuration._endpoint = _Configuration.DEFAULT_ENDPOINT;
_Configuration._token = null;
_Configuration._headers = {};
_Configuration._tenantId = null;
_Configuration._workspaceId = null;
_Configuration._source = _Configuration.DEFAULT_SOURCE;
_Configuration._sessionId = null;
_Configuration._retryBaseDelay = _Configuration.DEFAULT_RETRY_BASE_DELAY;
_Configuration._retryMaxDelay = _Configuration.DEFAULT_RETRY_MAX_DELAY;
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
  static addDeliveryListener(listener) {
    this._listeners.push(listener);
  }
  static notify(call) {
    for (const listener of this._listeners) {
      try {
        call(listener);
      } catch (error) {
        console.error("Delivery listener error:", error);
      }
    }
  }
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
    if (useBeacon) {
      this.sendAllWithKeepalive();
      return null;
    }
    if (this._isUploadInProgress || this.isBackingOff()) return null;
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
    const success = await this.sendNormally(payload);
    this._isUploadInProgress = false;
    if (!success) {
      this.notify((listener) => listener.onFailed?.(batch));
      this.scheduleRetry();
      return null;
    }
    this._failedAttempts = 0;
    this.notify((listener) => listener.onDelivered?.(batch));
    return batch;
  }
  static isBackingOff() {
    return Date.now() < this._retryAt;
  }
  /**
   * Back off exponentially (retryBaseDelay, doubling, capped at retryMaxDelay) with jitter so many
   * clients recovering from the same outage don't retry in lockstep, then retry.
   */
  static scheduleRetry() {
    this._failedAttempts++;
    const maxDelay = Math.min(
      Configuration.retryMaxDelay,
      Configuration.retryBaseDelay * 2 ** (this._failedAttempts - 1)
    );
    const delay = maxDelay / 2 + Math.random() * (maxDelay / 2);
    this._retryAt = Date.now() + delay;
    if (this._retryTimer !== null) clearTimeout(this._retryTimer);
    this._retryTimer = setTimeout(() => {
      this._retryTimer = null;
      this._retryAt = 0;
      flush(false).catch((err) => console.error("Retry flush error:", err));
    }, delay);
  }
  /**
   * Send all pending batches with keepalive so they survive page unload.
   * Batches leave the queue before sending so a later flush can't resend them;
   * if the page is still alive when a send fails, the batch is requeued.
   */
  static sendAllWithKeepalive() {
    const batches = Batcher.takeAllPending(this._isUploadInProgress);
    if (batches.length > 0) this.notify((listener) => listener.onUnloadSend?.(batches));
    for (const batch of batches) {
      void this.sendWithKeepalive(this.transformBatch(batch)).then((success) => {
        if (success) {
          this.notify((listener) => listener.onDelivered?.(batch));
        } else {
          this.notify((listener) => listener.onFailed?.(batch));
          Batcher.requeue(batch, this._isUploadInProgress);
          if (!this.isBackingOff()) this.scheduleRetry();
        }
      });
    }
  }
  static async sendWithKeepalive(payload) {
    if (typeof fetch === "undefined") {
      return false;
    }
    const headers = this.buildHeaders(payload);
    if (!headers) {
      return false;
    }
    try {
      const response = await fetch(this.endpoint, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
        keepalive: true
      });
      return response.ok;
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
    const tenantId = Configuration.tenantId ?? payload.events[0]?.tenant_id ?? null;
    if (!tenantId) {
      console.error("Cannot send analytics batch: tenantId is required by the collector.");
      return null;
    }
    const workspaceId = Configuration.workspaceId ?? payload.events[0]?.workspace_id ?? null;
    const headers = {
      "Content-Type": "application/json",
      ...Configuration.headers,
      "x-tenant-id": tenantId
    };
    if (workspaceId) {
      headers["x-workspace-id"] = workspaceId;
    }
    if (Configuration.token) {
      headers.Authorization = `Bearer ${Configuration.token}`;
    }
    return headers;
  }
  /**
   * Reset upload and backoff state. Useful for testing.
   */
  static reset() {
    if (this._retryTimer !== null) clearTimeout(this._retryTimer);
    this._retryTimer = null;
    this._retryAt = 0;
    this._failedAttempts = 0;
    this._isUploadInProgress = false;
    this._listeners = [];
  }
  static async startScheduler(time) {
    setInterval(() => {
      flush();
    }, time);
  }
};
Pusher._isUploadInProgress = false;
Pusher._failedAttempts = 0;
Pusher._retryAt = 0;
Pusher._retryTimer = null;
Pusher._listeners = [];

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
    if (this.accumulatingBatch().length >= Configuration.batchSize) {
      this.addNewBatch();
    }
    const lastBatch = this.accumulatingBatch();
    lastBatch.push(event);
    if (lastBatch.length === Configuration.batchSize) {
      flush(false).catch((err) => console.error("Auto-flush error:", err));
    }
  }
  static fetchBatchToUpload() {
    const batchToUpload = this.batches[0];
    if (batchToUpload.length == 0) return null;
    if (this.batches.length === 1) {
      this.addNewBatch();
    }
    return batchToUpload;
  }
  static addNewBatch() {
    this.batches.push([]);
  }
  static setMarkLastBatchUploaded() {
    this.batches.shift();
    if (this.batches.length === 0) {
      this.batches.push([]);
    }
  }
  /**
   * Remove every pending batch for a page-unload send. When keepHead is true the
   * head batch is already being uploaded, so it stays queued for that upload to mark.
   */
  static takeAllPending(keepHead) {
    const pending = this.batches.splice(keepHead ? 1 : 0).filter((batch) => batch.length > 0);
    this.batches.push([]);
    return pending;
  }
  /**
   * Put back a batch whose unload send failed so the next flush retries it.
   * When afterHead is true it goes behind the batch currently being uploaded.
   */
  static requeue(batch, afterHead) {
    this.batches.splice(afterHead ? 1 : 0, 0, batch);
  }
  /**
   * Queue events restored from storage ahead of the batch currently accumulating.
   */
  static addRestored(events) {
    const size = Math.max(1, Configuration.batchSize);
    for (let i = 0; i < events.length; i += size) {
      this.batches.splice(this.batches.length - 1, 0, events.slice(i, i + size));
    }
  }
  static accumulatingBatch() {
    return this.batches[this.batches.length - 1];
  }
  /**
   * Reset all internal state. Useful for testing.
   */
  static reset() {
    this.batches = [[]];
  }
};
// Queue of pending batches: the head is the next to upload, the tail is accumulating.
// Uploaded batches are removed so memory stays bounded by what is still unsent.
Batcher.batches = [[]];

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

// src/Sequence.ts
var streamId = null;
var nextSeq = 0;
function nextSequence() {
  if (!streamId) {
    streamId = generateUuid();
  }
  return { stream_id: streamId, seq: nextSeq++ };
}

// src/Outbox.ts
var KEY_PREFIX = "sudoquery_outbox:";
var OWNER_LOCK_PREFIX = "sudoquery_outbox_owner:";
var CLAIM_LOCK = "sudoquery_outbox_claim";
var DEFAULT_MAX_EVENTS = 1e3;
var DEFAULT_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1e3;
var Outbox = class {
  constructor(options = {}) {
    this.ownerId = generateUuid();
    this.key = KEY_PREFIX + this.ownerId;
    this.failing = false;
    this.hasStored = false;
    this.storage = options.storage !== void 0 ? options.storage : defaultStorage();
    this.locks = options.locks !== void 0 ? options.locks : defaultLocks();
    this.isOffline = options.isOffline ?? defaultIsOffline;
    this.maxEvents = options.maxEvents ?? DEFAULT_MAX_EVENTS;
    this.maxAgeMs = options.maxAgeMs ?? DEFAULT_MAX_AGE_MS;
    this.now = options.now ?? Date.now;
  }
  /**
   * Take ownership of this page's storage key and return events left behind by
   * earlier page loads. Returned events are removed from storage; they are stored
   * again if sending them fails.
   */
  async start() {
    if (!this.storage) return [];
    try {
      const locks = this.locks;
      if (!locks) return this.claim(/* @__PURE__ */ new Set());
      await this.holdOwnerLock(locks);
      return await locks.request(CLAIM_LOCK, async () => this.claim(await liveOwners(locks)));
    } catch (error) {
      console.error("Outbox restore failed:", error);
      return [];
    }
  }
  onFailed(batch) {
    this.failing = true;
    this.save(batch);
  }
  onDelivered(batch) {
    this.failing = false;
    if (!this.hasStored) return;
    const ids = new Set(batch.map((event) => event.id));
    const stored = this.read(this.key);
    const kept = stored.filter((event) => !ids.has(event.id));
    if (kept.length !== stored.length) this.write(kept);
  }
  onUnloadSend(batches) {
    if (this.failing || this.isOffline()) {
      this.save(batches.flat());
    }
  }
  holdOwnerLock(locks) {
    return new Promise((resolve) => {
      locks.request(OWNER_LOCK_PREFIX + this.ownerId, () => {
        resolve();
        return new Promise(() => {
        });
      }).catch(() => resolve());
    });
  }
  claim(liveOwnerIds) {
    const keys = this.outboxKeys().filter(
      (key) => key !== this.key && !liveOwnerIds.has(key.slice(KEY_PREFIX.length))
    );
    const events = [];
    for (const key of keys) {
      events.push(...this.read(key));
      try {
        this.storage?.removeItem(key);
      } catch (_) {
      }
    }
    return this.prune(events);
  }
  save(events) {
    if (!this.storage || events.length === 0) return;
    this.write(this.prune([...this.read(this.key), ...events]));
  }
  /**
   * Drop duplicates and expired events, order by time, and keep the newest maxEvents.
   */
  prune(events) {
    const oldest = this.now() - this.maxAgeMs;
    const seen = /* @__PURE__ */ new Set();
    const kept = [];
    for (const event of events) {
      const time = Date.parse(event.occured_at);
      if (seen.has(event.id) || !(time >= oldest)) continue;
      seen.add(event.id);
      kept.push({ event, time });
    }
    kept.sort((a, b) => a.time - b.time);
    return kept.slice(-this.maxEvents).map(({ event }) => event);
  }
  outboxKeys() {
    const keys = [];
    try {
      for (let i = 0; i < (this.storage?.length ?? 0); i++) {
        const key = this.storage?.key(i);
        if (key?.startsWith(KEY_PREFIX)) keys.push(key);
      }
    } catch (_) {
    }
    return keys;
  }
  read(key) {
    try {
      const parsed = JSON.parse(this.storage?.getItem(key) ?? "[]");
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }
  write(events) {
    try {
      if (events.length === 0) {
        this.storage?.removeItem(this.key);
      } else {
        this.storage?.setItem(this.key, JSON.stringify(events));
      }
      this.hasStored = events.length > 0;
    } catch (_) {
    }
  }
};
async function liveOwners(locks) {
  const snapshot = await locks.query();
  const ids = /* @__PURE__ */ new Set();
  for (const lock of snapshot.held ?? []) {
    if (lock.name?.startsWith(OWNER_LOCK_PREFIX)) {
      ids.add(lock.name.slice(OWNER_LOCK_PREFIX.length));
    }
  }
  return ids;
}
function defaultStorage() {
  try {
    return typeof window !== "undefined" && window.localStorage ? window.localStorage : null;
  } catch (_) {
    return null;
  }
}
function defaultLocks() {
  return typeof navigator !== "undefined" && navigator.locks ? navigator.locks : null;
}
function defaultIsOffline() {
  return typeof navigator !== "undefined" && navigator.onLine === false;
}

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
    if (config?.tenantId !== void 0) {
      Configuration.setTenantId(config.tenantId);
    }
    if (config?.workspaceId !== void 0) {
      Configuration.setWorkspaceId(config.workspaceId);
    }
    if (config?.source !== void 0) {
      Configuration.setSource(config.source);
    }
    if (config?.sessionId !== void 0) {
      Configuration.setSessionId(config.sessionId);
    }
    if (config?.retryBaseDelay !== void 0 && config.retryBaseDelay > 0) {
      Configuration.setRetryBaseDelay(config.retryBaseDelay);
    }
    if (config?.retryMaxDelay !== void 0 && config.retryMaxDelay > 0) {
      Configuration.setRetryMaxDelay(config.retryMaxDelay);
    }
    if (config?.persistence) {
      this.enablePersistence();
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
  /**
   * Store undelivered events in browser storage and send them on a later page load.
   */
  static enablePersistence() {
    const outbox = new Outbox();
    Pusher.addDeliveryListener(outbox);
    outbox.start().then((events) => {
      if (events.length === 0) return;
      Batcher.addRestored(events);
      return this.flush(false);
    }).catch((err) => console.error("Outbox restore error:", err));
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
    const tenantId = Configuration.tenantId;
    if (!tenantId || tenantId.trim().length === 0) {
      throw new Error("tenantId is required before tracking events");
    }
    const superProperties = SuperProperties.getSuperProperties();
    const mergedProperties = mergeProperties(props, superProperties);
    const event = {
      envelop_version: "1.0",
      id: generateUuid(),
      name: eventName.toString(),
      tenant_id: tenantId,
      workspace_id: Configuration.workspaceId,
      session_id: Configuration.sessionId ?? getSessionId(),
      anon_id: AnonymousId.getOrCreate(),
      actor_id: this.currentUser,
      source: Configuration.source,
      occured_at: (/* @__PURE__ */ new Date()).toISOString(),
      properties: mergedProperties,
      correlation_id: null,
      trace_id: null,
      system_properties: null,
      ...nextSequence()
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
function isJsonRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
export {
  SudoQuery
};
