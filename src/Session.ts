import type { SessionData } from "./types";
import { getDeviceInfo, getPlatform, isPlatformInitialized } from "./platform";
import { detectBrowser } from "./platform/browser/DeviceInfo.js";

/**
 * Collects session information from the current environment.
 * Returns default values for fields that cannot be determined client-side.
 */
export async function getSessionData(): Promise<SessionData> {
  if (!isPlatformInitialized()) {
    return getDefaultSessionData();
  }

  const platform = getPlatform();
  const deviceInfo = getDeviceInfo();

  if (platform === 'react-native') {
    const [deviceType, platformName, osVersion, appVersion] = await Promise.all([
      deviceInfo.getDeviceType(),
      deviceInfo.getPlatform(),
      deviceInfo.getOSVersion(),
      deviceInfo.getAppVersion(),
    ]);

    return {
      device_type: deviceType,
      platform: platformName,
      browser: `${platformName} ${osVersion}`, // No browser in RN
      country: "",
      city: "",
      ip_address: null,
      user_agent: `${platformName}/${osVersion} App/${appVersion}`,
    };
  }

  // Browser and Node.js: use userAgent
  const userAgent = deviceInfo.getUserAgent();
  return {
    device_type: await deviceInfo.getDeviceType(),
    platform: await deviceInfo.getPlatform(),
    browser: detectBrowser(userAgent),
    country: "",
    city: "",
    ip_address: null,
    user_agent: userAgent,
  };
}

/**
 * Get default session data when platform is not initialized.
 */
function getDefaultSessionData(): SessionData {
  return {
    device_type: "unknown",
    platform: "unknown",
    browser: "unknown",
    country: "",
    city: "",
    ip_address: null,
    user_agent: "unknown",
  };
}