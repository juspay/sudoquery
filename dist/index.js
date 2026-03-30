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

// src/TypeValidator.ts
function containsNonPrimitives(obj) {
  if (obj === null || typeof obj !== "object") {
    return false;
  }
  if (Array.isArray(obj)) {
    return false;
  }
  for (const key in obj) {
    if (Object.prototype.hasOwnProperty.call(obj, key)) {
      const value = obj[key];
      if (typeof value === "object" && value !== null) {
        return true;
      }
    }
  }
  return false;
}

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
};
_Configuration._batchSize = 10;
_Configuration._flushInterval = null;
_Configuration._endpoint = "https://sudoquery.juspay.io/api/push_batch";
_Configuration._token = null;
var Configuration = _Configuration;

// src/Session.ts
function getSessionData() {
  const userAgent = typeof navigator !== "undefined" ? navigator.userAgent : "Unknown";
  return {
    device_type: detectDeviceType(userAgent),
    platform: detectPlatform(userAgent),
    browser: detectBrowser(userAgent),
    country: "",
    city: "",
    ip_address: null,
    user_agent: userAgent
  };
}
function detectDeviceType(userAgent) {
  const ua = userAgent.toLowerCase();
  if (/ipad|android(?!.*mobile)|tablet|kindle|silk/i.test(ua)) {
    return "tablet";
  }
  if (/mobile|android|iphone|ipod|blackberry|opera mini|iemobile|wpdesktop/i.test(ua)) {
    return "mobile";
  }
  return "desktop";
}
function detectPlatform(userAgent) {
  const ua = userAgent.toLowerCase();
  if (ua.includes("windows")) return "Windows";
  if (ua.includes("mac os x") || ua.includes("macintosh")) return "macOS";
  if (ua.includes("linux")) return "Linux";
  if (ua.includes("android")) return "Android";
  if (ua.includes("ios") || ua.includes("iphone") || ua.includes("ipad") || ua.includes("ipod")) return "iOS";
  return "Unknown";
}
function detectBrowser(userAgent) {
  const ua = userAgent.toLowerCase();
  if (ua.includes("firefox") && !ua.includes("seamonkey")) return "Firefox";
  if (ua.includes("seamonkey")) return "SeaMonkey";
  if (ua.includes("chrome") && !ua.includes("chromium") && !ua.includes("edge") && !ua.includes("opr")) return "Chrome";
  if (ua.includes("chromium")) return "Chromium";
  if (ua.includes("safari") && !ua.includes("chrome") && !ua.includes("chromium")) return "Safari";
  if (ua.includes("opr") || ua.includes("opera")) return "Opera";
  if (ua.includes("edge") || ua.includes("edg")) return "Edge";
  if (ua.includes("trident") || ua.includes("msie")) return "Internet Explorer";
  return "Unknown";
}

// src/Pusher.ts
var Pusher = class {
  static get endpoint() {
    return Configuration.endpoint;
  }
  /**
   * Transform internal Event array to BatchPayload format
   */
  static transformBatch(batch) {
    const sessionData = getSessionData();
    const clientEvents = batch.map((event) => ({
      event_id: event.eventId,
      event_name: event.eventName,
      event_timestamp: event.at,
      user_id: event.user,
      anon_id: event.anon_id,
      properties: JSON.stringify(event.properties)
    }));
    return {
      session: sessionData,
      events: clientEvents
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
      this.sendWithBeacon(payload);
      this._isUploadInProgress = false;
      return null;
    } else {
      const success = await this.sendNormally(payload);
      this._isUploadInProgress = false;
      return success ? batch : null;
    }
  }
  static sendWithBeacon(payload) {
    if (typeof navigator !== "undefined" && navigator.sendBeacon) {
      try {
        const blob = new Blob([JSON.stringify(payload)], {
          type: "application/json"
        });
        return navigator.sendBeacon(this.endpoint, blob);
      } catch (error) {
        console.error("Beacon send failed:", error);
        return false;
      }
    }
    return false;
  }
  static async sendNormally(payload) {
    try {
      const headers = {
        "Content-Type": "application/json"
      };
      if (Configuration.token) {
        headers["Authorization"] = `Bearer ${Configuration.token}`;
      }
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
  /**
   * Generates a new UUID v4 using crypto.randomUUID()
   */
  static generateId() {
    return crypto.randomUUID();
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
      let anonId = localStorage.getItem(STORAGE_KEY);
      if (!anonId) {
        anonId = this.generateId();
        localStorage.setItem(STORAGE_KEY, anonId);
      }
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
      localStorage.removeItem(STORAGE_KEY);
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
      return localStorage.getItem(STORAGE_KEY);
    } else {
      return this.inMemoryAnonId;
    }
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
    if (config?.flushInterval !== void 0 && config.flushInterval > 0) {
      Configuration.setFlushInterval(config.flushInterval);
      this.startPeriodicFlush(config.flushInterval);
    }
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") {
          this.stopPeriodicFlush();
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
  static stopPeriodicFlush() {
    if (this.flushTimer !== null) {
      clearInterval(this.flushTimer);
      this.flushTimer = null;
    }
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
   * @param useBeacon - Use navigator.sendBeacon for more reliable delivery during page unload
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
    if (containsNonPrimitives(props)) throw new Error("only primitives are allowed as properties");
    const sessionDetails = {};
    const superProperties = SuperProperties.getSuperProperties();
    const mergedProperties = {
      ...superProperties,
      ...typeof props === "object" && props !== null && !Array.isArray(props) ? props : {},
      ...sessionDetails
    };
    const event = {
      eventName: eventName.toString(),
      properties: mergedProperties,
      user: this.currentUser,
      anon_id: AnonymousId.getOrCreate(),
      eventId: crypto.randomUUID(),
      at: Date.now()
    };
    Batcher.addToBatch(event);
  }
};
SudoQuery.didInit = false;
SudoQuery.currentUser = null;
SudoQuery.flushTimer = null;
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  SudoQuery
});
