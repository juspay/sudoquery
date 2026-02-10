import type { SessionData } from "./types";

/**
 * Collects session information from the browser environment.
 * Returns default values for fields that cannot be determined client-side.
 */
export function getSessionData(): SessionData {
  const userAgent = typeof navigator !== "undefined"
    ? navigator.userAgent
    : "Unknown";

  return {
    device_type: detectDeviceType(userAgent),
    platform: detectPlatform(userAgent),
    browser: detectBrowser(userAgent),
    country: "",
    city: "",
    ip_address: null,
    user_agent: userAgent,
  };
}

/**
 * Detects device type from user agent string.
 * Returns 'desktop', 'mobile', 'tablet', or 'unknown'.
 */
function detectDeviceType(userAgent: string): string {
  const ua = userAgent.toLowerCase();

  // Check for tablet devices
  if (/ipad|android(?!.*mobile)|tablet|kindle|silk/i.test(ua)) {
    return "tablet";
  }

  // Check for mobile devices
  if (/mobile|android|iphone|ipod|blackberry|opera mini|iemobile|wpdesktop/i.test(ua)) {
    return "mobile";
  }

  // Default to desktop
  return "desktop";
}

/**
 * Detects operating system/platform from user agent string.
 */
function detectPlatform(userAgent: string): string {
  const ua = userAgent.toLowerCase();

  if (ua.includes("windows")) return "Windows";
  if (ua.includes("mac os x") || ua.includes("macintosh")) return "macOS";
  if (ua.includes("linux")) return "Linux";
  if (ua.includes("android")) return "Android";
  if (ua.includes("ios") || ua.includes("iphone") || ua.includes("ipad") || ua.includes("ipod")) return "iOS";

  return "Unknown";
}

/**
 * Detects browser from user agent string.
 */
function detectBrowser(userAgent: string): string {
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