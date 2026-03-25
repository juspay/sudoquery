// src/platform/react-native/Storage.ts
var AsyncStorage = null;
async function getAsyncStorage() {
  if (AsyncStorage) return AsyncStorage;
  try {
    const module = await import("@react-native-async-storage/async-storage").catch(() => null);
    if (module) {
      AsyncStorage = module.default || module;
      return AsyncStorage;
    }
  } catch {
  }
  return null;
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
var AppState = null;
var AppStatePromise = null;
function getAppState() {
  if (AppState) return Promise.resolve(AppState);
  if (AppStatePromise) return AppStatePromise;
  AppStatePromise = (async () => {
    try {
      const rn = await import("react-native").catch(() => null);
      if (rn?.AppState) {
        AppState = rn.AppState;
        return AppState;
      }
    } catch {
    }
    return null;
  })();
  return AppStatePromise;
}
function createLifecycle() {
  return {
    onBackground(callback) {
      let subscription = null;
      getAppState().then((appState) => {
        if (appState) {
          subscription = appState.addEventListener("change", (state) => {
            if (state === "background" || state === "inactive") {
              callback();
            }
          });
        }
      }).catch(() => {
      });
      return () => {
        subscription?.remove();
      };
    },
    onForeground(callback) {
      let subscription = null;
      getAppState().then((appState) => {
        if (appState) {
          subscription = appState.addEventListener("change", (state) => {
            if (state === "active") {
              callback();
            }
          });
        }
      }).catch(() => {
      });
      return () => {
        subscription?.remove();
      };
    },
    onTerminate(callback) {
      let subscription = null;
      getAppState().then((appState) => {
        if (appState) {
          subscription = appState.addEventListener("change", (state) => {
            if (state === "background") {
              callback();
            }
          });
        }
      }).catch(() => {
      });
      return () => {
        subscription?.remove();
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
  try {
    const [expoDevice, expoApplication] = await Promise.all([
      import("expo-device").catch(() => null),
      import("expo-application").catch(() => null)
    ]);
    if (expoDevice) {
      deviceInfo = {
        async getDeviceType() {
          const type = expoDevice.deviceType;
          const typeMap = {
            1: "unknown",
            2: "mobile",
            3: "tablet",
            4: "desktop",
            5: "tv"
          };
          return typeMap[type] || "mobile";
        },
        async getPlatform() {
          return expoDevice.osName || "Unknown";
        },
        async getOSVersion() {
          return expoDevice.osVersion || "";
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
  } catch {
  }
  try {
    const rnDeviceInfo = await import("react-native-device-info").catch(() => null);
    if (rnDeviceInfo) {
      const info = rnDeviceInfo.default || rnDeviceInfo;
      deviceInfo = {
        async getDeviceType() {
          const type = await info.getDeviceType();
          return type.toLowerCase();
        },
        async getPlatform() {
          return info.getSystemName();
        },
        async getOSVersion() {
          return info.getSystemVersion();
        },
        async getAppVersion() {
          return `${info.getVersion()} (${info.getBuildNumber()})`;
        },
        getUserAgent() {
          return "ReactNative";
        }
      };
      return deviceInfo;
    }
  } catch {
  }
  deviceInfo = {
    async getDeviceType() {
      return "mobile";
    },
    async getPlatform() {
      return "Unknown";
    },
    async getOSVersion() {
      return "";
    },
    async getAppVersion() {
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
