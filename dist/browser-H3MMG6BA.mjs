import {
  createDeviceInfo,
  detectBrowser
} from "./chunk-7Y3ODE36.mjs";

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
export {
  createDeviceInfo,
  createLifecycle,
  createNetwork,
  createStorage,
  detectBrowser
};
