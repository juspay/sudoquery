/**
 * Platform abstraction layer types.
 * Defines interfaces that each platform must implement.
 */

/**
 * Storage adapter for persistent data.
 * Used for anonymous ID and pending events.
 */
export interface StorageAdapter {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
}

/**
 * Lifecycle adapter for app state changes.
 * Handles background, foreground, and termination events.
 */
export interface LifecycleAdapter {
  /** Register callback for when app goes to background */
  onBackground(callback: () => void): () => void;
  /** Register callback for when app comes to foreground */
  onForeground(callback: () => void): () => void;
  /** Register callback for app termination (best-effort) */
  onTerminate(callback: () => void): () => void;
}

/**
 * Network adapter for HTTP requests.
 * Handles normal and unreliable (page unload/terminate) delivery.
 */
export interface NetworkAdapter {
  /** Send request with full error handling */
  send(url: string, payload: unknown, headers: Record<string, string>): Promise<boolean>;
  /** Send request without waiting for response (fire-and-forget) */
  sendUnreliable(url: string, payload: unknown, headers?: Record<string, string>): boolean;
}

/**
 * Device info adapter for platform detection.
 * Provides device and OS information.
 */
export interface DeviceInfoAdapter {
  /** Get device type: 'mobile', 'tablet', 'desktop', etc. */
  getDeviceType(): Promise<string>;
  /** Get platform name: 'iOS', 'Android', 'Windows', etc. */
  getPlatform(): Promise<string>;
  /** Get OS version */
  getOSVersion(): Promise<string>;
  /** Get app version (mobile only) */
  getAppVersion(): Promise<string>;
  /** Get user agent string */
  getUserAgent(): string;
}

/**
 * Platform type identifier.
 */
export type PlatformType = 'browser' | 'react-native' | 'node';
