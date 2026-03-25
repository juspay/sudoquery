/**
 * React Native device info adapter.
 * Supports:
 * - expo-device + expo-application (works in Expo Go)
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

  // Try expo-device + expo-application first (works in Expo Go)
  try {
    const [expoDevice, expoApplication] = await Promise.all([
      import('expo-device').catch(() => null),
      import('expo-application').catch(() => null),
    ]);

    if (expoDevice) {
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
          if (expoApplication) {
            const version = expoApplication.nativeApplicationVersion || '';
            const build = expoApplication.nativeBuildVersion || '';
            return build ? `${version} (${build})` : version;
          }
          return '';
        },

        getUserAgent(): string {
          return 'ReactNative';
        },
      };

      return deviceInfo;
    }
  } catch {
    // expo packages not available
  }

  // Try react-native-device-info (for bare React Native)
  try {
    const rnDeviceInfo = await import('react-native-device-info').catch(() => null);
    if (rnDeviceInfo) {
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
    }
  } catch {
    // react-native-device-info not available
  }

  // Fallback: return minimal info without importing react-native
  // to avoid native module access errors in Expo Go
  deviceInfo = {
    async getDeviceType(): Promise<string> { return 'mobile'; },
    async getPlatform(): Promise<string> { return 'Unknown'; },
    async getOSVersion(): Promise<string> { return ''; },
    async getAppVersion(): Promise<string> { return ''; },
    getUserAgent(): string { return 'ReactNative'; },
  };

  return deviceInfo;
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