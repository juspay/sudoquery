/**
 * React Native storage adapter using AsyncStorage.
 * Gracefully handles Expo Go and bare React Native.
 */
import type { StorageAdapter } from '../types';

// Type for AsyncStorage
type AsyncStorageType = {
  getItem: (key: string) => Promise<string | null>;
  setItem: (key: string, value: string) => Promise<void>;
  removeItem: (key: string) => Promise<void>;
};

let AsyncStorage: AsyncStorageType | null = null;

async function getAsyncStorage(): Promise<AsyncStorageType | null> {
  if (AsyncStorage) return AsyncStorage;

  try {
    const module = await import('@react-native-async-storage/async-storage').catch(() => null);
    if (module) {
      AsyncStorage = module.default || module;
      return AsyncStorage;
    }
  } catch {
    // AsyncStorage not available
  }

  return null;
}

// In-memory fallback for when AsyncStorage is not available
const memoryStorage = new Map<string, string>();

export function createStorage(): StorageAdapter {
  return {
    async getItem(key: string): Promise<string | null> {
      const storage = await getAsyncStorage();
      if (storage) {
        return storage.getItem(key);
      }
      // Fallback to memory storage
      return memoryStorage.get(key) ?? null;
    },

    async setItem(key: string, value: string): Promise<void> {
      const storage = await getAsyncStorage();
      if (storage) {
        await storage.setItem(key, value);
      } else {
        // Fallback to memory storage
        memoryStorage.set(key, value);
      }
    },

    async removeItem(key: string): Promise<void> {
      const storage = await getAsyncStorage();
      if (storage) {
        await storage.removeItem(key);
      } else {
        // Fallback to memory storage
        memoryStorage.delete(key);
      }
    },
  };
}