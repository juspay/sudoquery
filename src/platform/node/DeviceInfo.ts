/**
 * Node.js device info adapter (minimal info).
 */
import type { DeviceInfoAdapter } from '../types';

export function createDeviceInfo(): DeviceInfoAdapter {
  return {
    async getDeviceType(): Promise<string> {
      return 'server';
    },

    async getPlatform(): Promise<string> {
      return process.platform || 'node';
    },

    async getOSVersion(): Promise<string> {
      return (process.release as { version?: string })?.version || '';
    },

    async getAppVersion(): Promise<string> {
      return '';
    },

    getUserAgent(): string {
      return `Node.js/${process.version}`;
    },
  };
}
