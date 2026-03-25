declare module '@react-native-async-storage/async-storage' {
  export interface AsyncStorageStatic {
    getItem(key: string): Promise<string | null>;
    setItem(key: string, value: string): Promise<void>;
    removeItem(key: string): Promise<void>;
    clear(): Promise<void>;
    getAllKeys(): Promise<readonly string[]>;
    multiGet(keys: readonly string[]): Promise<readonly [string, string | null][]>;
    multiSet(keyValuePairs: readonly [string, string][]): Promise<void>;
    multiRemove(keys: readonly string[]): Promise<void>;
  }
  const AsyncStorage: AsyncStorageStatic;
  export default AsyncStorage;
}

declare module 'react-native' {
  export type AppStateStatus = 'active' | 'background' | 'inactive' | 'unknown' | 'extension';

  export interface AppStateStatic {
    currentState: AppStateStatus;
    addEventListener(type: 'change', handler: (newAppState: AppStateStatus) => void): { remove: () => void };
    removeEventListener(type: 'change', handler: (newAppState: AppStateStatus) => void): void;
  }

  export const AppState: AppStateStatic;

  export interface PlatformStatic {
    OS: 'android' | 'ios' | 'macos' | 'windows' | 'web' | 'native';
    Version: string | number;
    select<T>(specifics: { [platform in 'android' | 'ios' | 'macos' | 'windows' | 'web' | 'native']?: T }): T | undefined;
  }

  export const Platform: PlatformStatic;
}

declare module 'react-native-device-info' {
  export type DeviceType = 'Handset' | 'Tablet' | 'Tv' | 'Desktop' | 'unknown';

  export interface DeviceInfoStatic {
    getDeviceType(): Promise<DeviceType>;
    getSystemName(): string;
    getSystemVersion(): string;
    getVersion(): string;
    getBuildNumber(): string;
    getBrand(): string;
    getModel(): string;
    getDeviceId(): string;
    getDeviceName(): Promise<string>;
    getFirstInstallTime(): Promise<number>;
    getLastUpdateTime(): Promise<number>;
    isEmulator(): Promise<boolean>;
  }

  const DeviceInfo: DeviceInfoStatic;
  export default DeviceInfo;
}
