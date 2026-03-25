/**
 * Browser device info adapter using navigator.userAgent.
 */
import type { DeviceInfoAdapter } from '../types';

export function createDeviceInfo(): DeviceInfoAdapter {
  return {
    async getDeviceType(): Promise<string> {
      const userAgent = this.getUserAgent();
      return detectDeviceType(userAgent);
    },

    async getPlatform(): Promise<string> {
      const userAgent = this.getUserAgent();
      return detectPlatform(userAgent);
    },

    async getOSVersion(): Promise<string> {
      const userAgent = this.getUserAgent();
      return detectOSVersion(userAgent);
    },

    async getAppVersion(): Promise<string> {
      // Browser doesn't have app version
      return '';
    },

    getUserAgent(): string {
      if (typeof navigator !== 'undefined') {
        return navigator.userAgent;
      }
      return 'Unknown';
    },
  };
}

/**
 * Detects device type from user agent string.
 */
function detectDeviceType(userAgent: string): string {
  const ua = userAgent.toLowerCase();

  // Check for tablet devices
  if (/ipad|android(?!.*mobile)|tablet|kindle|silk/i.test(ua)) {
    return 'tablet';
  }

  // Check for mobile devices
  if (
    /mobile|android|iphone|ipod|blackberry|opera mini|iemobile|wpdesktop/i.test(
      ua
    )
  ) {
    return 'mobile';
  }

  // Default to desktop
  return 'desktop';
}

/**
 * Detects operating system/platform from user agent string.
 */
function detectPlatform(userAgent: string): string {
  const ua = userAgent.toLowerCase();

  if (ua.includes('windows')) return 'Windows';
  if (ua.includes('mac os x') || ua.includes('macintosh')) return 'macOS';
  if (ua.includes('linux')) return 'Linux';
  if (ua.includes('android')) return 'Android';
  if (
    ua.includes('ios') ||
    ua.includes('iphone') ||
    ua.includes('ipad') ||
    ua.includes('ipod')
  )
    return 'iOS';

  return 'Unknown';
}

/**
 * Detects browser from user agent string.
 */
export function detectBrowser(userAgent: string): string {
  const ua = userAgent.toLowerCase();

  if (ua.includes('firefox') && !ua.includes('seamonkey')) return 'Firefox';
  if (ua.includes('seamonkey')) return 'SeaMonkey';
  if (
    ua.includes('chrome') &&
    !ua.includes('chromium') &&
    !ua.includes('edge') &&
    !ua.includes('opr')
  )
    return 'Chrome';
  if (ua.includes('chromium')) return 'Chromium';
  if (
    ua.includes('safari') &&
    !ua.includes('chrome') &&
    !ua.includes('chromium')
  )
    return 'Safari';
  if (ua.includes('opr') || ua.includes('opera')) return 'Opera';
  if (ua.includes('edge') || ua.includes('edg')) return 'Edge';
  if (ua.includes('trident') || ua.includes('msie')) return 'Internet Explorer';

  return 'Unknown';
}

/**
 * Detects OS version from user agent string (best effort).
 */
function detectOSVersion(userAgent: string): string {
  const ua = userAgent.toLowerCase();

  // iOS
  const iosMatch = ua.match(/os (\d+)[._](\d+)/);
  if (iosMatch) return `${iosMatch[1]}.${iosMatch[2]}`;

  // Android
  const androidMatch = ua.match(/android (\d+)[._](\d+)?/);
  if (androidMatch) {
    return androidMatch[2]
      ? `${androidMatch[1]}.${androidMatch[2]}`
      : androidMatch[1];
  }

  // Windows
  const windowsMatch = ua.match(/windows nt (\d+)[._](\d+)/);
  if (windowsMatch) return `${windowsMatch[1]}.${windowsMatch[2]}`;

  // macOS
  const macMatch = ua.match(/mac os x (\d+)[._](\d+)/);
  if (macMatch) return `${macMatch[1]}.${macMatch[2]}`;

  return '';
}
