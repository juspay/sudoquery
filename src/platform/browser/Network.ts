/**
 * Browser network adapter using fetch and sendBeacon.
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
      // Use sendBeacon for reliable delivery during page unload
      if (typeof navigator !== 'undefined' && navigator.sendBeacon) {
        try {
          // Note: sendBeacon doesn't support custom headers
          // For authenticated requests, include token in payload or URL
          const blob = new Blob([JSON.stringify(payload)], {
            type: 'application/json',
          });
          return navigator.sendBeacon(url, blob);
        } catch (error) {
          console.error('Beacon send failed:', error);
          return false;
        }
      }

      // Fallback to fetch with keepalive
      if (typeof fetch !== 'undefined') {
        fetch(url, {
          method: 'POST',
          headers: headers || { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
          keepalive: true,
        }).catch(() => {}); // Silent fail for unreliable delivery
        return true;
      }

      return false;
    },
  };
}
