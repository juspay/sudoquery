/**
 * React Native device info adapter.
 * Supports:
 * - expo-device (works in Expo Go)
 * - expo-application (works in Expo Go)
 * - react-native-device-info (for bare React Native)
 * Falls back to basic info if none are available.
 */
import type { DeviceInfoAdapter } from '../types';

interface DeviceInfo {
  getDeviceType(): Promise<string>;
  getPlatform(): Promise<string>;
  getOSVersion(): Promise<string>;
  getAppVersion(): Promise<string>;
  getUserAgent(): string;
}

let deviceInfo: DeviceInfo | null = null;

async function getDeviceInfo(): Promise<DeviceInfo> {
  if (deviceInfo) return deviceInfo;

  // Try expo-device first (works in Expo Go)
  try {
    const expoDevice = await import('expo-device');
    const expoApplication = await import('expo-application');

    deviceInfo = {
      async getDeviceType(): Promise<string> {
        const type = expoDevice.deviceType;
        // expo-device returns: 1=Unknown, 2=Phone, 3=Tablet, 4=Desktop, 5=TV
        const typeMap: Record<number, string> = {
          1: 'unknown',
          2: 'mobile',
          3: 'tablet',
          4: 'desktop',
          5: 'tv',
        };
        return typeMap[type] || 'mobile';
      },

      async getPlatform(): Promise<string> {
        return expoDevice.osName || 'Unknown';
      },

      async getOSVersion(): Promise<string> {
        return expoDevice.osVersion || '';
      },

      async getAppVersion(): Promise<string> {
        const version = expoApplication.nativeApplicationVersion || '';
        const build = expoApplication.nativeBuildVersion || '';
        return build ? `${version} (${build})` : version;
      },

      getUserAgent(): string {
        return 'ReactNative';
      },
    };

    return deviceInfo;
  } catch {
    // expo-device not available, try react-native-device-info
  }

  // Try react-native-device-info (for bare React Native)
  try {
    const rnDeviceInfo = await import('react-native-device-info');
    const info = rnDeviceInfo.default || rnDeviceInfo;

    deviceInfo = {
      async getDeviceType(): Promise<string> {
        const type = await info.getDeviceType();
        return type.toLowerCase();
      },

      async getPlatform(): Promise<string> {
        return info.getSystemName();
      },

      async getOSVersion(): Promise<string> {
        return info.getSystemVersion();
      },

      async getAppVersion(): Promise<string> {
        return `${info.getVersion()} (${info.getBuildNumber()})`;
      },

      getUserAgent(): string {
        return 'ReactNative';
      },
    };

    return deviceInfo;
  } catch {
    // react-native-device-info not available either
  }

  // Fallback: use React Native's Platform API (always available)
  try {
    const { Platform } = await import('react-native');

    deviceInfo = {
      async getDeviceType(): Promise<string> {
        // Best guess based on platform
        return 'mobile';
      },

      async getPlatform(): Promise<string> {
        // Platform.OS returns 'ios' or 'android'
        return Platform.OS === 'ios' ? 'iOS' : 'Android';
      },

      async getOSVersion(): Promise<string> {
        return Platform.Version?.toString() || '';
      },

      async getAppVersion(): Promise<string> {
        // Can't get app version without expo-application or react-native-device-info
        return '';
      },

      getUserAgent(): string {
        return 'ReactNative';
      },
    };

    return deviceInfo;
  } catch {
    // Final fallback
    deviceInfo = {
      async getDeviceType(): Promise<string> { return 'mobile'; },
      async getPlatform(): Promise<string> { return 'Unknown'; },
      async getOSVersion(): Promise<string> { return ''; },
      async getAppVersion(): Promise<string> { return ''; },
      getUserAgent(): string { return 'ReactNative'; },
    };

    return deviceInfo;
  }
}

export function createDeviceInfo(): DeviceInfoAdapter {
  return {
    async getDeviceType(): Promise<string> {
      const info = await getDeviceInfo();
      return info.getDeviceType();
    },

    async getPlatform(): Promise<string> {
      const info = await getDeviceInfo();
      return info.getPlatform();
    },

    async getOSVersion(): Promise<string> {
      const info = await getDeviceInfo();
      return info.getOSVersion();
    },

    async getAppVersion(): Promise<string> {
      const info = await getDeviceInfo();
      return info.getAppVersion();
    },

    getUserAgent(): string {
      return 'ReactNative';
    },
  };
}