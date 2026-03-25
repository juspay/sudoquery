// src/platform/node/Storage.ts
var memoryStorage = /* @__PURE__ */ new Map();
function createStorage() {
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
function createLifecycle() {
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
function createDeviceInfo() {
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
export {
  createDeviceInfo,
  createLifecycle,
  createNetwork,
  createStorage
};
