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
  HyperAnalytics: () => HyperAnalytics,
  detectPlatform: () => detectPlatform2,
  getPlatform: () => getPlatform,
  isPlatformInitialized: () => isPlatformInitialized,
  resetPlatform: () => resetPlatform
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
_Configuration._endpoint = "http://hyper-analytics-alb-c33157e-1810523293.ap-south-1.elb.amazonaws.com/push_batch";
_Configuration._token = null;
var Configuration = _Configuration;

// src/platform/browser/Storage.ts
function createStorage() {
  return {
    getItem(key) {
      if (typeof localStorage === "undefined") {
        return Promise.resolve(null);
      }
      return Promise.resolve(localStorage.getItem(key));
    },
    setItem(key, value) {
      if (typeof localStorage === "undefined") {
        return Promise.resolve();
      }
      localStorage.setItem(key, value);
      return Promise.resolve();
    },
    removeItem(key) {
      if (typeof localStorage === "undefined") {
        return Promise.resolve();
      }
      localStorage.removeItem(key);
      return Promise.resolve();
    }
  };
}

// src/platform/browser/Lifecycle.ts
function createLifecycle() {
  return {
    onBackground(callback) {
      if (typeof document === "undefined") {
        return () => {
        };
      }
      const handler = () => {
        if (document.visibilityState === "hidden") {
          callback();
        }
      };
      document.addEventListener("visibilitychange", handler);
      return () => document.removeEventListener("visibilitychange", handler);
    },
    onForeground(callback) {
      if (typeof document === "undefined") {
        return () => {
        };
      }
      const handler = () => {
        if (document.visibilityState === "visible") {
          callback();
        }
      };
      document.addEventListener("visibilitychange", handler);
      return () => document.removeEventListener("visibilitychange", handler);
    },
    onTerminate(callback) {
      if (typeof document === "undefined") {
        return () => {
        };
      }
      const handler = () => {
        if (document.visibilityState === "hidden") {
          callback();
        }
      };
      document.addEventListener("visibilitychange", handler);
      window.addEventListener("pagehide", callback);
      return () => {
        document.removeEventListener("visibilitychange", handler);
        window.removeEventListener("pagehide", callback);
      };
    }
  };
}

// src/platform/browser/Network.ts
function createNetwork() {
  return {
    async send(url, payload, headers) {
      try {
        const response = await fetch(url, {
          method: "POST",
          headers,
          body: JSON.stringify(payload)
        });
        return response.ok;
      } catch (error) {
        console.error("Network send failed:", error);
        return false;
      }
    },
    sendUnreliable(url, payload, headers) {
      if (typeof navigator !== "undefined" && navigator.sendBeacon) {
        try {
          const blob = new Blob([JSON.stringify(payload)], {
            type: "application/json"
          });
          return navigator.sendBeacon(url, blob);
        } catch (error) {
          console.error("Beacon send failed:", error);
          return false;
        }
      }
      if (typeof fetch !== "undefined") {
        fetch(url, {
          method: "POST",
          headers: headers || { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
          keepalive: true
        }).catch(() => {
        });
        return true;
      }
      return false;
    }
  };
}

// src/platform/browser/DeviceInfo.ts
function createDeviceInfo() {
  return {
    async getDeviceType() {
      const userAgent = this.getUserAgent();
      return detectDeviceType(userAgent);
    },
    async getPlatform() {
      const userAgent = this.getUserAgent();
      return detectPlatform(userAgent);
    },
    async getOSVersion() {
      const userAgent = this.getUserAgent();
      return detectOSVersion(userAgent);
    },
    async getAppVersion() {
      return "";
    },
    getUserAgent() {
      if (typeof navigator !== "undefined") {
        return navigator.userAgent;
      }
      return "Unknown";
    }
  };
}
function detectDeviceType(userAgent) {
  const ua = userAgent.toLowerCase();
  if (/ipad|android(?!.*mobile)|tablet|kindle|silk/i.test(ua)) {
    return "tablet";
  }
  if (/mobile|android|iphone|ipod|blackberry|opera mini|iemobile|wpdesktop/i.test(
    ua
  )) {
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
  if (ua.includes("ios") || ua.includes("iphone") || ua.includes("ipad") || ua.includes("ipod"))
    return "iOS";
  return "Unknown";
}
function detectBrowser(userAgent) {
  const ua = userAgent.toLowerCase();
  if (ua.includes("firefox") && !ua.includes("seamonkey")) return "Firefox";
  if (ua.includes("seamonkey")) return "SeaMonkey";
  if (ua.includes("chrome") && !ua.includes("chromium") && !ua.includes("edge") && !ua.includes("opr"))
    return "Chrome";
  if (ua.includes("chromium")) return "Chromium";
  if (ua.includes("safari") && !ua.includes("chrome") && !ua.includes("chromium"))
    return "Safari";
  if (ua.includes("opr") || ua.includes("opera")) return "Opera";
  if (ua.includes("edge") || ua.includes("edg")) return "Edge";
  if (ua.includes("trident") || ua.includes("msie")) return "Internet Explorer";
  return "Unknown";
}
function detectOSVersion(userAgent) {
  const ua = userAgent.toLowerCase();
  const iosMatch = ua.match(/os (\d+)[._](\d+)/);
  if (iosMatch) return `${iosMatch[1]}.${iosMatch[2]}`;
  const androidMatch = ua.match(/android (\d+)[._](\d+)?/);
  if (androidMatch) {
    return androidMatch[2] ? `${androidMatch[1]}.${androidMatch[2]}` : androidMatch[1];
  }
  const windowsMatch = ua.match(/windows nt (\d+)[._](\d+)/);
  if (windowsMatch) return `${windowsMatch[1]}.${windowsMatch[2]}`;
  const macMatch = ua.match(/mac os x (\d+)[._](\d+)/);
  if (macMatch) return `${macMatch[1]}.${macMatch[2]}`;
  return "";
}

// src/platform/node/Storage.ts
var memoryStorage = /* @__PURE__ */ new Map();
function createStorage2() {
  return {
    getItem(key) {
      return Promise.resolve(memoryStorage.get(key) ?? null);
    },
    setItem(key, value) {
      memoryStorage.set(key, value);
      return Promise.resolve();
    },
    removeItem(key) {
      memoryStorage.delete(key);
      return Promise.resolve();
    }
  };
}

// src/platform/node/Lifecycle.ts
function createLifecycle2() {
  return {
    onBackground(_callback) {
      return () => {
      };
    },
    onForeground(_callback) {
      return () => {
      };
    },
    onTerminate(_callback) {
      return () => {
      };
    }
  };
}

// src/platform/node/Network.ts
function createNetwork2() {
  return {
    async send(url, payload, headers) {
      try {
        const response = await fetch(url, {
          method: "POST",
          headers,
          body: JSON.stringify(payload)
        });
        return response.ok;
      } catch (error) {
        console.error("Network send failed:", error);
        return false;
      }
    },
    sendUnreliable(url, payload, headers) {
      fetch(url, {
        method: "POST",
        headers: headers || { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      }).catch(() => {
      });
      return true;
    }
  };
}

// src/platform/node/DeviceInfo.ts
function createDeviceInfo2() {
  return {
    async getDeviceType() {
      return "server";
    },
    async getPlatform() {
      return process.platform || "node";
    },
    async getOSVersion() {
      return process.release?.version || "";
    },
    async getAppVersion() {
      return "";
    },
    getUserAgent() {
      return `Node.js/${process.version}`;
    }
  };
}

// src/platform/react-native/Storage.ts
var AsyncStorage = null;
async function getAsyncStorage() {
  if (AsyncStorage) return AsyncStorage;
  try {
    const module2 = require("@react-native-async-storage/async-storage");
    AsyncStorage = module2.default || module2;
    return AsyncStorage;
  } catch {
    throw new Error(
      "@react-native-async-storage/async-storage is required for React Native. Install it with: npm install @react-native-async-storage/async-storage"
    );
  }
}
function createStorage3() {
  return {
    async getItem(key) {
      const storage2 = await getAsyncStorage();
      return storage2.getItem(key);
    },
    async setItem(key, value) {
      const storage2 = await getAsyncStorage();
      await storage2.setItem(key, value);
    },
    async removeItem(key) {
      const storage2 = await getAsyncStorage();
      await storage2.removeItem(key);
    }
  };
}

// src/platform/react-native/Lifecycle.ts
var AppState = null;
var AppStatePromise = null;
function getAppState() {
  if (AppState) return Promise.resolve(AppState);
  if (AppStatePromise) return AppStatePromise;
  AppStatePromise = new Promise((resolve, reject) => {
    try {
      const rn = require("react-native");
      AppState = rn.AppState;
      if (AppState) {
        resolve(AppState);
      } else {
        reject(new Error("AppState not available in react-native"));
      }
    } catch {
      reject(new Error(
        "react-native is required. Install it with: npm install react-native"
      ));
    }
  });
  return AppStatePromise;
}
function createLifecycle3() {
  return {
    onBackground(callback) {
      let subscription = null;
      getAppState().then((appState) => {
        subscription = appState.addEventListener("change", (state) => {
          if (state === "background" || state === "inactive") {
            callback();
          }
        });
      }).catch(() => {
      });
      return () => {
        subscription?.remove();
      };
    },
    onForeground(callback) {
      let subscription = null;
      getAppState().then((appState) => {
        subscription = appState.addEventListener("change", (state) => {
          if (state === "active") {
            callback();
          }
        });
      }).catch(() => {
      });
      return () => {
        subscription?.remove();
      };
    },
    onTerminate(callback) {
      let subscription = null;
      getAppState().then((appState) => {
        subscription = appState.addEventListener("change", (state) => {
          if (state === "background") {
            callback();
          }
        });
      }).catch(() => {
      });
      return () => {
        subscription?.remove();
      };
    }
  };
}

// src/platform/react-native/Network.ts
function createNetwork3() {
  return {
    async send(url, payload, headers) {
      try {
        const response = await fetch(url, {
          method: "POST",
          headers,
          body: JSON.stringify(payload)
        });
        return response.ok;
      } catch (error) {
        console.error("Network send failed:", error);
        return false;
      }
    },
    sendUnreliable(url, payload, headers) {
      fetch(url, {
        method: "POST",
        headers: headers || { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      }).catch(() => {
      });
      return true;
    }
  };
}

// src/platform/react-native/DeviceInfo.ts
var DeviceInfo = null;
var DeviceInfoPromise = null;
function getDeviceInfo() {
  if (DeviceInfo) return Promise.resolve(DeviceInfo);
  if (DeviceInfoPromise) return DeviceInfoPromise;
  DeviceInfoPromise = new Promise((resolve, reject) => {
    try {
      const module2 = require("react-native-device-info");
      DeviceInfo = module2.default || module2;
      if (DeviceInfo) {
        resolve(DeviceInfo);
      } else {
        reject(new Error("DeviceInfo not available in react-native-device-info"));
      }
    } catch {
      reject(new Error(
        "react-native-device-info is required for React Native. Install it with: npm install react-native-device-info"
      ));
    }
  });
  return DeviceInfoPromise;
}
function createDeviceInfo3() {
  return {
    async getDeviceType() {
      const info = await getDeviceInfo();
      const type = await info.getDeviceType();
      return type.toLowerCase();
    },
    async getPlatform() {
      const info = await getDeviceInfo();
      return info.getSystemName();
    },
    async getOSVersion() {
      const info = await getDeviceInfo();
      return info.getSystemVersion();
    },
    async getAppVersion() {
      const info = await getDeviceInfo();
      return `${info.getVersion()} (${info.getBuildNumber()})`;
    },
    getUserAgent() {
      return "ReactNative";
    }
  };
}

// src/platform/index.ts
var storage = null;
var lifecycle = null;
var network = null;
var deviceInfo = null;
var currentPlatform = null;
function detectPlatform2() {
  if (typeof navigator !== "undefined" && navigator.product === "ReactNative") {
    return "react-native";
  }
  if (typeof document !== "undefined" && typeof window !== "undefined") {
    return "browser";
  }
  return "node";
}
function getPlatform() {
  if (!currentPlatform) {
    throw new Error("Platform not initialized. Call initializePlatform() first.");
  }
  return currentPlatform;
}
function isPlatformInitialized() {
  return currentPlatform !== null;
}
function resetPlatform() {
  storage = null;
  lifecycle = null;
  network = null;
  deviceInfo = null;
  currentPlatform = null;
}
async function initializePlatform() {
  if (currentPlatform) return;
  currentPlatform = detectPlatform2();
  if (currentPlatform === "react-native") {
    storage = createStorage3();
    lifecycle = createLifecycle3();
    network = createNetwork3();
    deviceInfo = createDeviceInfo3();
  } else if (currentPlatform === "browser") {
    storage = createStorage();
    lifecycle = createLifecycle();
    network = createNetwork();
    deviceInfo = createDeviceInfo();
  } else {
    storage = createStorage2();
    lifecycle = createLifecycle2();
    network = createNetwork2();
    deviceInfo = createDeviceInfo2();
  }
}
function getStorage() {
  if (!storage) {
    throw new Error("Platform not initialized. Call initializePlatform() first.");
  }
  return storage;
}
function getLifecycle() {
  if (!lifecycle) {
    throw new Error("Platform not initialized. Call initializePlatform() first.");
  }
  return lifecycle;
}
function getNetwork() {
  if (!network) {
    throw new Error("Platform not initialized. Call initializePlatform() first.");
  }
  return network;
}
function getDeviceInfo2() {
  if (!deviceInfo) {
    throw new Error("Platform not initialized. Call initializePlatform() first.");
  }
  return deviceInfo;
}

// src/Session.ts
async function getSessionData() {
  if (!isPlatformInitialized()) {
    return getDefaultSessionData();
  }
  const platform = getPlatform();
  const deviceInfo2 = getDeviceInfo2();
  if (platform === "react-native") {
    const [deviceType, platformName, osVersion, appVersion] = await Promise.all([
      deviceInfo2.getDeviceType(),
      deviceInfo2.getPlatform(),
      deviceInfo2.getOSVersion(),
      deviceInfo2.getAppVersion()
    ]);
    return {
      device_type: deviceType,
      platform: platformName,
      browser: `${platformName} ${osVersion}`,
      // No browser in RN
      country: "",
      city: "",
      ip_address: null,
      user_agent: `${platformName}/${osVersion} App/${appVersion}`
    };
  }
  const userAgent = deviceInfo2.getUserAgent();
  return {
    device_type: await deviceInfo2.getDeviceType(),
    platform: await deviceInfo2.getPlatform(),
    browser: detectBrowser(userAgent),
    country: "",
    city: "",
    ip_address: null,
    user_agent: userAgent
  };
}
function getDefaultSessionData() {
  return {
    device_type: "unknown",
    platform: "unknown",
    browser: "unknown",
    country: "",
    city: "",
    ip_address: null,
    user_agent: "unknown"
  };
}

// src/Pusher.ts
var Pusher = class {
  static get endpoint() {
    return Configuration.endpoint;
  }
  /**
   * Get headers for API requests.
   */
  static getHeaders() {
    const headers = {
      "Content-Type": "application/json"
    };
    if (Configuration.token) {
      headers["Authorization"] = `Bearer ${Configuration.token}`;
    }
    return headers;
  }
  /**
   * Transform internal Event array to BatchPayload format
   */
  static async transformBatch(batch) {
    const sessionData = await getSessionData();
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
    const payload = await this.transformBatch(batch);
    if (!payload) {
      this._isUploadInProgress = false;
      return null;
    }
    if (useBeacon && isPlatformInitialized()) {
      const network2 = getNetwork();
      const headers = this.getHeaders();
      network2.sendUnreliable(this.endpoint, payload, headers);
      this._isUploadInProgress = false;
      return null;
    } else if (isPlatformInitialized()) {
      const network2 = getNetwork();
      const headers = this.getHeaders();
      const success = await network2.send(this.endpoint, payload, headers);
      this._isUploadInProgress = false;
      return success ? batch : null;
    } else {
      this._isUploadInProgress = false;
      return null;
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
var PENDING_EVENTS_KEY = "hyper_analytics_pending_events";
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
   * Get all pending events (for persistence).
   */
  static getAllPendingEvents() {
    const allEvents = [];
    for (const batch of this.batches) {
      allEvents.push(...batch);
    }
    return allEvents;
  }
  /**
   * Clear all pending events (after successful persistence).
   */
  static clearAllEvents() {
    this.batches = [[]];
    this._currentBatchToUpload = 0;
    this.currentAccumilatingBatch = 0;
  }
  /**
   * Persist pending events to storage (for React Native termination handling).
   */
  static async persistBatch() {
    if (!isPlatformInitialized()) return;
    const platform = getPlatform();
    if (platform !== "react-native") return;
    const storage2 = getStorage();
    const pendingEvents = this.getAllPendingEvents();
    if (pendingEvents.length > 0) {
      await storage2.setItem(PENDING_EVENTS_KEY, JSON.stringify(pendingEvents));
    }
  }
  /**
   * Restore pending events from storage (on app launch).
   */
  static async restoreBatch() {
    if (!isPlatformInitialized()) return;
    const platform = getPlatform();
    if (platform !== "react-native") return;
    const storage2 = getStorage();
    const pending = await storage2.getItem(PENDING_EVENTS_KEY);
    if (pending) {
      try {
        const events = JSON.parse(pending);
        for (const event of events) {
          this.addToBatch(event);
        }
        await storage2.removeItem(PENDING_EVENTS_KEY);
      } catch (error) {
        console.error("Failed to restore pending events:", error);
      }
    }
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
   * Generates a new UUID v4.
   * Works in browser, React Native, and Node.js.
   */
  static generateId() {
    if (typeof crypto !== "undefined" && crypto.randomUUID) {
      return crypto.randomUUID();
    }
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = Math.random() * 16 | 0;
      const v = c === "x" ? r : r & 3 | 8;
      return v.toString(16);
    });
  }
  /**
   * Initialize the anonymous ID from storage.
   * Must be called before getOrCreate() in React Native.
   */
  static async initialize() {
    if (this.initPromise) return this.initPromise;
    this.initPromise = (async () => {
      if (this.cachedAnonId) return;
      if (isPlatformInitialized()) {
        try {
          const storage2 = getStorage();
          let anonId = await storage2.getItem(STORAGE_KEY);
          if (!anonId) {
            anonId = this.generateId();
            await storage2.setItem(STORAGE_KEY, anonId);
          }
          this.cachedAnonId = anonId;
        } catch {
          this.cachedAnonId = this.generateId();
        }
      } else {
        this.cachedAnonId = this.generateId();
      }
    })();
    return this.initPromise;
  }
  /**
   * Gets the current anonymous ID.
   * Synchronous version - returns cached ID.
   *
   * @returns The anonymous ID string
   * @throws Error if not initialized (call init() first)
   */
  static getOrCreate() {
    if (this.cachedAnonId) return this.cachedAnonId;
    throw new Error(
      "AnonymousId not initialized. Call HyperAnalytics.init() first."
    );
  }
  /**
   * Gets the current anonymous ID (async version).
   * Initializes if not already done.
   *
   * @returns The anonymous ID string
   */
  static async getOrCreateAsync() {
    await this.initialize();
    return this.cachedAnonId;
  }
  /**
   * Resets the anonymous ID.
   * Removes from storage and clears memory.
   */
  static async reset() {
    this.cachedAnonId = null;
    this.initPromise = null;
    if (isPlatformInitialized()) {
      try {
        const storage2 = getStorage();
        await storage2.removeItem(STORAGE_KEY);
      } catch {
      }
    }
  }
  /**
   * Sync reset - clears memory only.
   * Used when resetting without async context.
   */
  static resetSync() {
    this.cachedAnonId = null;
    this.initPromise = null;
  }
  /**
   * Gets the current anonymous ID without creating a new one if it doesn't exist.
   *
   * @returns The anonymous ID string, or null if not set
   */
  static get() {
    return this.cachedAnonId;
  }
};
// In-memory cache for all environments
AnonymousId.cachedAnonId = null;
AnonymousId.initPromise = null;

// src/HyperAnalytics.ts
var HyperAnalytics = class {
  /**
   * Initialize the analytics SDK.
   * This method is async for React Native support (storage initialization).
   *
   * @param config - Configuration options
   */
  static async init(config) {
    if (this.didInit) return;
    await initializePlatform();
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
    await AnonymousId.initialize();
    await Batcher.restoreBatch();
    this.setupLifecycleHandlers();
    this.didInit = true;
  }
  /**
   * Set up platform-specific lifecycle handlers.
   */
  static setupLifecycleHandlers() {
    if (!isPlatformInitialized()) return;
    const platform = getPlatform();
    const lifecycle2 = getLifecycle();
    if (platform === "react-native") {
      const cleanupBackground = lifecycle2.onBackground(async () => {
        this.stopPeriodicFlush();
        await this.flush(true);
        await Batcher.persistBatch();
      });
      const cleanupForeground = lifecycle2.onForeground(() => {
        if (Configuration.flushInterval) {
          this.startPeriodicFlush(Configuration.flushInterval);
        }
      });
      this.cleanupFns.push(cleanupBackground, cleanupForeground);
    } else if (platform === "browser") {
      const cleanupBackground = lifecycle2.onBackground(() => {
        this.stopPeriodicFlush();
        this.flush(true).catch((err) => console.error("Flush on hidden error:", err));
      });
      this.cleanupFns.push(cleanupBackground);
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
   * @param useBeacon - Use unreliable delivery for page unload/app background
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
      eventId: this.generateEventId(),
      at: Date.now()
    };
    Batcher.addToBatch(event);
  }
  /**
   * Generate a unique event ID.
   */
  static generateEventId() {
    if (typeof crypto !== "undefined" && crypto.randomUUID) {
      return crypto.randomUUID();
    }
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = Math.random() * 16 | 0;
      const v = c === "x" ? r : r & 3 | 8;
      return v.toString(16);
    });
  }
  /**
   * Reset the SDK state. Useful for testing or logging out.
   */
  static async reset() {
    this.stopPeriodicFlush();
    this.cleanupFns.forEach((fn) => fn());
    this.cleanupFns = [];
    this.currentUser = null;
    this.didInit = false;
    Batcher.reset();
    AnonymousId.resetSync();
    resetPlatform();
  }
};
HyperAnalytics.didInit = false;
HyperAnalytics.currentUser = null;
HyperAnalytics.flushTimer = null;
HyperAnalytics.cleanupFns = [];
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  HyperAnalytics,
  detectPlatform,
  getPlatform,
  isPlatformInitialized,
  resetPlatform
});
