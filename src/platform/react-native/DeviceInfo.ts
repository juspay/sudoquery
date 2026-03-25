/**
 * React Native device info adapter.
 * Uses only expo-device and expo-application which work in Expo Go.
 * NO react-native or react-native-device-info imports to avoid native module errors.
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

  let expoDevice: typeof import('expo-device') | null = null;
  let expoApplication: typeof import('expo-application') | null = null;

  // Try to load expo-device (works in Expo Go)
  try {
    expoDevice = await import('expo-device');
  } catch {
    // expo-device not available
  }

  // Try to load expo-application (works in Expo Go)
  try {
    expoApplication = await import('expo-application');
  } catch {
    // expo-application not available
  }

  deviceInfo = {
    async getDeviceType(): Promise<string> {
      if (expoDevice) {
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
      }
      return 'mobile';
    },

    async getPlatform(): Promise<string> {
      if (expoDevice?.osName) {
        return expoDevice.osName;
      }
      return 'Unknown';
    },

    async getOSVersion(): Promise<string> {
      if (expoDevice?.osVersion) {
        return expoDevice.osVersion;
      }
      return '';
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