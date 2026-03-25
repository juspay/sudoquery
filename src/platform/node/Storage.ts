/**
 * Node.js storage adapter (in-memory only).
 */
import type { StorageAdapter } from '../types';

// In-memory storage for Node.js environment
const memoryStorage = new Map<string, string>();

export function createStorage(): StorageAdapter {
  return {
    getItem(key: string): Promise<string | null> {
      return Promise.resolve(memoryStorage.get(key) ?? null);
    },

    setItem(key: string, value: string): Promise<void> {
      memoryStorage.set(key, value);
      return Promise.resolve();
    },

    removeItem(key: string): Promise<void> {
      memoryStorage.delete(key);
      return Promise.resolve();
    },
  };
}
