// Type declarations for React Native packages
// Only includes packages that work in Expo Go

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

declare module 'expo-device' {
  export const deviceType: number;
  export const osName: string | null;
  export const osVersion: string | null;
  export const deviceName: string | null;
  export const brand: string | null;
  export const modelName: string | null;

  export const DeviceType: {
    Unknown: 1;
    Phone: 2;
    Tablet: 3;
    Desktop: 4;
    TV: 5;
  };
}

declare module 'expo-application' {
  export const nativeApplicationVersion: string | null;
  export const nativeBuildVersion: string | null;
  export const applicationName: string | null;
  export const applicationId: string | null;
}