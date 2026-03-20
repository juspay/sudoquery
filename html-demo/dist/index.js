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
  Batcher: () => Batcher,
  Configuration: () => Configuration,
  HyperAnalytics: () => HyperAnalytics,
  Pusher: () => Pusher,
  SuperProperties: () => SuperProperties,
  add: () => add,
  containsNonPrimitives: () => containsNonPrimitives,
  flush: () => flush,
  logMessage: () => logMessage
});
module.exports = __toCommonJS(index_exports);

// src/TypeValidator.ts
function containsNonPrimitives(obj) {
  if (obj === null || typeof obj !== "object") {
    return false;
  }
  if (Array.isArray(obj)) {
    for (const item of obj) {
      if (typeof item === "object" && item !== null) {
        return true;
      }
    }
  } else {
    for (const key in obj) {
      if (Object.prototype.hasOwnProperty.call(obj, key)) {
        const value = obj[key];
        if (typeof value === "object" && value !== null) {
          return true;
        }
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
  static set batchSize(value) {
    _Configuration._batchSize = value;
  }
};
_Configuration._batchSize = 10;
var Configuration = _Configuration;

// src/Pusher.ts
var Pusher = class {
  static setEndpoint(url) {
    this.endpoint = url;
  }
  static async pushLogs(useBeacon = false) {
    if (this._isUploadInProgress) return null;
    this._isUploadInProgress = true;
    const batch = Batcher.fetchBatchToUpload();
    if (!batch) {
      this._isUploadInProgress = false;
      return null;
    }
    if (useBeacon) {
      this.sendWithBeacon(batch);
      this._isUploadInProgress = false;
      return null;
    } else {
      const success = await this.sendNormally(batch);
      this._isUploadInProgress = false;
      return success ? batch : null;
    }
  }
  static sendWithBeacon(batch) {
    if (typeof navigator !== "undefined" && navigator.sendBeacon) {
      try {
        const blob = new Blob([JSON.stringify(batch)], {
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
  static async sendNormally(batch) {
    try {
      const response = await fetch(this.endpoint, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(batch)
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
Pusher.endpoint = "http://localhost:8000/events/batch";

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

// src/HyperAnalytics.ts
var HyperAnalytics = class {
  static init(clientId) {
    if (this.didInit) return;
    this.didInit = true;
    this.clientId = clientId;
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") {
          flush(true).catch((err) => console.error("Flush on pagehide error:", err));
        }
      });
    }
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
  /**
   * Set the group ID for all subsequent events
   * @param groupId - The group identifier
   */
  static setGroup(groupId) {
    this.currentGroup = groupId;
  }
  /**
   * Remove the current group ID (resets to null)
   */
  static removeGroup() {
    this.currentGroup = null;
  }
  /**
   * Get the current group ID
   * @returns The current group ID or null if not set
   */
  static getGroup() {
    return this.currentGroup;
  }
  static track(eventName, properties) {
    if (containsNonPrimitives(properties)) throw new Error("only primitives are allowed as properties");
    if (this.clientId === null) return;
    const sessionDetails = {};
    const superProperties = SuperProperties.getSuperProperties();
    const mergedProperties = {
      ...superProperties,
      ...typeof properties === "object" && properties !== null && !Array.isArray(properties) ? properties : {},
      ...sessionDetails
    };
    const event = {
      clientId: this.clientId,
      eventName: eventName.toString(),
      properties: mergedProperties,
      user: this.currentUser,
      group: this.currentGroup,
      at: Date.now()
    };
    Batcher.addToBatch(event);
  }
};
HyperAnalytics.didInit = false;
HyperAnalytics.currentUser = null;
HyperAnalytics.currentGroup = null;
HyperAnalytics.clientId = null;

// src/index.ts
var add = (a, b) => {
  return a + b;
};
var logMessage = (msg) => {
  console.log(`[MyLib]: ${msg}`);
};
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  Batcher,
  Configuration,
  HyperAnalytics,
  Pusher,
  SuperProperties,
  add,
  containsNonPrimitives,
  flush,
  logMessage
});
