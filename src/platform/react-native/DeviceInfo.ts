/**
 * React Native device info adapter using react-native-device-info.
 */
import type { DeviceInfoAdapter } from '../types';

interface RNDeviceInfo {
  getDeviceType: () => Promise<string>;
  getSystemName: () => string;
  getSystemVersion: () => string;
  getVersion: () => string;
  getBuildNumber: () => string;
}

let DeviceInfo: RNDeviceInfo | null = null;
let DeviceInfoPromise: Promise<RNDeviceInfo> | null = null;

function getDeviceInfo(): Promise<RNDeviceInfo> {
  if (DeviceInfo) return Promise.resolve(DeviceInfo);
  if (DeviceInfoPromise) return DeviceInfoPromise;

  DeviceInfoPromise = (async () => {
    try {
      const module = await import('react-native-device-info');
      DeviceInfo = module.default || module;
      if (!DeviceInfo) {
        throw new Error('DeviceInfo not available in react-native-device-info');
      }
      return DeviceInfo;
    } catch {
      throw new Error(
        'react-native-device-info is required for React Native. ' +
          'Install it with: npm install react-native-device-info'
      );
    }
  })();

  return DeviceInfoPromise;
}

export function createDeviceInfo(): DeviceInfoAdapter {
  return {
    async getDeviceType(): Promise<string> {
      const info = await getDeviceInfo();
      const type = await info.getDeviceType();
      // Returns 'Handset', 'Tablet', 'Tv', 'Desktop', 'unknown'
      return type.toLowerCase();
    },

    async getPlatform(): Promise<string> {
      const info = await getDeviceInfo();
      return info.getSystemName(); // 'iOS', 'Android', etc.
    },

    async getOSVersion(): Promise<string> {
      const info = await getDeviceInfo();
      return info.getSystemVersion();
    },

    async getAppVersion(): Promise<string> {
      const info = await getDeviceInfo();
      return `${info.getVersion()} (${info.getBuildNumber()})`;
    },

    getUserAgent(): string {
      // React Native doesn't have traditional userAgent
      // Will be built from device info
      return 'ReactNative';
    },
  };
}