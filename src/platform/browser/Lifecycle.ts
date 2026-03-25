/**
 * Browser lifecycle adapter using document.visibilitychange.
 */
import type { LifecycleAdapter } from '../types';

export function createLifecycle(): LifecycleAdapter {
  return {
    onBackground(callback: () => void): () => void {
      if (typeof document === 'undefined') {
        return () => {};
      }

      const handler = (): void => {
        if (document.visibilityState === 'hidden') {
          callback();
        }
      };

      document.addEventListener('visibilitychange', handler);
      return () => document.removeEventListener('visibilitychange', handler);
    },

    onForeground(callback: () => void): () => void {
      if (typeof document === 'undefined') {
        return () => {};
      }

      const handler = (): void => {
        if (document.visibilityState === 'visible') {
          callback();
        }
      };

      document.addEventListener('visibilitychange', handler);
      return () => document.removeEventListener('visibilitychange', handler);
    },

    onTerminate(callback: () => void): () => void {
      // Browser doesn't have a true terminate event
      // Use visibilitychange with hidden state as approximation
      if (typeof document === 'undefined') {
        return () => {};
      }

      const handler = (): void => {
        if (document.visibilityState === 'hidden') {
          callback();
        }
      };

      document.addEventListener('visibilitychange', handler);
      // Also listen for pagehide as a backup
      window.addEventListener('pagehide', callback);

      return () => {
        document.removeEventListener('visibilitychange', handler);
        window.removeEventListener('pagehide', callback);
      };
    },
  };
}
