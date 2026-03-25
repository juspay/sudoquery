/**
 * Node.js lifecycle adapter (no-op, no lifecycle events).
 */
import type { LifecycleAdapter } from '../types';

export function createLifecycle(): LifecycleAdapter {
  return {
    onBackground(_callback: () => void): () => void {
      // No lifecycle events in Node.js
      return () => {};
    },

    onForeground(_callback: () => void): () => void {
      // No lifecycle events in Node.js
      return () => {};
    },

    onTerminate(_callback: () => void): () => void {
      // Could listen for process exit, but it's not reliable for async operations
      return () => {};
    },
  };
}
