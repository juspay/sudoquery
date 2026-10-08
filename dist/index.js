"use strict";
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// src/index.ts
var index_exports = {};
__export(index_exports, {
  SudoQuery: () => SudoQuery
});
module.exports = __toCommonJS(index_exports);

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
  }
};
_Configuration.DEFAULT_ENDPOINT = "http://localhost:3000/batch";
_Configuration.DEFAULT_SOURCE = "typescript";
_Configuration._batchSize = 10;
_Configuration._flushInterval = null;
_Configuration._endpoint = _Configuration.DEFAULT_ENDPOINT;
_Configuration._token = null;
_Configuration._headers = {};
_Configuration._tenantId = null;
_Configuration._workspaceId = null;
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
function isJsonRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  SudoQuery
});
