// src/platform/react-native/Storage.ts
var AsyncStorage = null;
var storagePromise = null;
async function getAsyncStorage() {
  if (AsyncStorage) return AsyncStorage;
  if (storagePromise) return storagePromise;
  storagePromise = (async () => {
    try {
      const module = await import("@react-native-async-storage/async-storage");
      AsyncStorage = module.default || module;
      return AsyncStorage;
    } catch {
      return null;
    }
  })();
  return storagePromise;
}
var memoryStorage = /* @__PURE__ */ new Map();
function createStorage() {
  return {
    async getItem(key) {
      const storage = await getAsyncStorage();
      if (storage) {
        return storage.getItem(key);
      }
      return memoryStorage.get(key) ?? null;
    },
    async setItem(key, value) {
      const storage = await getAsyncStorage();
      if (storage) {
        await storage.setItem(key, value);
      } else {
        memoryStorage.set(key, value);
      }
    },
    async removeItem(key) {
      const storage = await getAsyncStorage();
      if (storage) {
        await storage.removeItem(key);
      } else {
        memoryStorage.delete(key);
      }
    }
  };
}

// src/platform/react-native/Lifecycle.ts
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

// src/platform/react-native/Network.ts
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

// src/platform/react-native/DeviceInfo.ts
var deviceInfo = null;
async function getDeviceInfo() {
  if (deviceInfo) return deviceInfo;
  let expoDevice = null;
  let expoApplication = null;
  try {
    expoDevice = await import("expo-device");
  } catch {
  }
  try {
    expoApplication = await import("expo-application");
  } catch {
  }
  deviceInfo = {
    async getDeviceType() {
      if (expoDevice) {
        const type = expoDevice.deviceType;
        const typeMap = {
          1: "unknown",
          2: "mobile",
          3: "tablet",
          4: "desktop",
          5: "tv"
        };
        return typeMap[type] || "mobile";
      }
      return "mobile";
    },
    async getPlatform() {
      if (expoDevice?.osName) {
        return expoDevice.osName;
      }
      return "Unknown";
    },
    async getOSVersion() {
      if (expoDevice?.osVersion) {
        return expoDevice.osVersion;
      }
      return "";
    },
    async getAppVersion() {
      if (expoApplication) {
        const version = expoApplication.nativeApplicationVersion || "";
        const build = expoApplication.nativeBuildVersion || "";
        return build ? `${version} (${build})` : version;
      }
      return "";
    },
    getUserAgent() {
      return "ReactNative";
    }
  };
  return deviceInfo;
}
function createDeviceInfo() {
  return {
    async getDeviceType() {
      const info = await getDeviceInfo();
      return info.getDeviceType();
    },
    async getPlatform() {
      const info = await getDeviceInfo();
      return info.getPlatform();
    },
    async getOSVersion() {
      const info = await getDeviceInfo();
      return info.getOSVersion();
    },
    async getAppVersion() {
      const info = await getDeviceInfo();
      return info.getAppVersion();
    },
    getUserAgent() {
      return "ReactNative";
    }
  };
}
export {
  createDeviceInfo,
  createLifecycle,
  createNetwork,
  createStorage
};
