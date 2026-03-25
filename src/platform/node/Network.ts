/**
 * Node.js network adapter using fetch.
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
      // In Node.js, just use regular fetch without waiting
      fetch(url, {
        method: 'POST',
        headers: headers || { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      }).catch(() => {}); // Silent fail for unreliable delivery
      return true;
    },
  };
}
