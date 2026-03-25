/**
 * React Native storage adapter using AsyncStorage.
 */
import type { StorageAdapter } from '../types';

// Type for AsyncStorage
type AsyncStorageType = {
  getItem: (key: string) => Promise<string | null>;
  setItem: (key: string, value: string) => Promise<void>;
  removeItem: (key: string) => Promise<void>;
};

let AsyncStorage: AsyncStorageType | null = null;

async function getAsyncStorage(): Promise<AsyncStorageType> {
  if (AsyncStorage) return AsyncStorage;

  try {
    const module = await import('@react-native-async-storage/async-storage');
    AsyncStorage = module.default || module;
    return AsyncStorage!;
  } catch {
    throw new Error(
      '@react-native-async-storage/async-storage is required for React Native. ' +
        'Install it with: npm install @react-native-async-storage/async-storage'
    );
  }
}

export function createStorage(): StorageAdapter {
  return {
    async getItem(key: string): Promise<string | null> {
      const storage = await getAsyncStorage();
      return storage.getItem(key);
    },

    async setItem(key: string, value: string): Promise<void> {
      const storage = await getAsyncStorage();
      await storage.setItem(key, value);
    },

    async removeItem(key: string): Promise<void> {
      const storage = await getAsyncStorage();
      await storage.removeItem(key);
    },
  };
}