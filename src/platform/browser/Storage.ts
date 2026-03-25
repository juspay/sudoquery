/**
 * Browser storage adapter using localStorage.
 */
import type { StorageAdapter } from '../types';

export function createStorage(): StorageAdapter {
  return {
    getItem(key: string): Promise<string | null> {
      if (typeof localStorage === 'undefined') {
        return Promise.resolve(null);
      }
      return Promise.resolve(localStorage.getItem(key));
    },

    setItem(key: string, value: string): Promise<void> {
      if (typeof localStorage === 'undefined') {
        return Promise.resolve();
      }
      localStorage.setItem(key, value);
      return Promise.resolve();
    },

    removeItem(key: string): Promise<void> {
      if (typeof localStorage === 'undefined') {
        return Promise.resolve();
      }
      localStorage.removeItem(key);
      return Promise.resolve();
    },
  };
}
