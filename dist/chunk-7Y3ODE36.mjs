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

export {
  createDeviceInfo,
  detectBrowser
};
