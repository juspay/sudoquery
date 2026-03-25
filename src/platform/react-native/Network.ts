/**
 * React Native network adapter using fetch.
 */
import type { NetworkAdapter } from '../types';

export function createNetwork(): NetworkAdapter {
  return {
    async send(
      url: string,
      payload: unknown,
      headers: Record<string, string>
    ): Promise<boolean> {
      try {
        const response = await fetch(url, {
          method: 'POST',
          headers,
          body: JSON.stringify(payload),
        });
        return response.ok;
      } catch (error) {
        console.error('Network send failed:', error);
        return false;
      }
    },

    sendUnreliable(
      url: string,
      payload: unknown,
      headers?: Record<string, string>
    ): boolean {
      // React Native doesn't have sendBeacon
      // Fire-and-forget with fetch (no await, no error handling)
      fetch(url, {
        method: 'POST',
        headers: headers || { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      }).catch(() => {}); // Silent fail

      return true; // Can't guarantee delivery
    },
  };
}