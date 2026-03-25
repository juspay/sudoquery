/**
 * Platform abstraction layer.
 * Detects runtime environment and provides appropriate adapters.
 * Uses dynamic imports to avoid bundling native modules in Expo Go.
 */

import type {
  StorageAdapter,
  LifecycleAdapter,
  NetworkAdapter,
  DeviceInfoAdapter,
  PlatformType,
} from './types';

export type { StorageAdapter, LifecycleAdapter, NetworkAdapter, DeviceInfoAdapter, PlatformType };

// Singleton adapters
let storage: StorageAdapter | null = null;
let lifecycle: LifecycleAdapter | null = null;
let network: NetworkAdapter | null = null;
let deviceInfo: DeviceInfoAdapter | null = null;
let currentPlatform: PlatformType | null = null;

/**
 * Detect the current runtime platform.
 */
export function detectPlatform(): PlatformType {
  // Check React Native first (has window but also native modules)
  if (typeof navigator !== 'undefined' && navigator.product === 'ReactNative') {
    return 'react-native';
  }
  // Check for browser environment
  if (typeof document !== 'undefined' && typeof window !== 'undefined') {
    return 'browser';
  }
  // Default to Node.js
  return 'node';
}

/**
 * Get the detected platform (throws if not initialized).
 */
export function getPlatform(): PlatformType {
  if (!currentPlatform) {
    throw new Error('Platform not initialized. Call initializePlatform() first.');
  }
  return currentPlatform;
}

/**
 * Check if platform has been initialized.
 */
export function isPlatformInitialized(): boolean {
  return currentPlatform !== null;
}

/**
 * Reset platform state. Used for testing.
 */
export function resetPlatform(): void {
  storage = null;
  lifecycle = null;
  network = null;
  deviceInfo = null;
  currentPlatform = null;
}

/**
 * Initialize the platform abstraction layer.
 * Uses dynamic imports to avoid bundling native modules unnecessarily.
 */
export async function initializePlatform(): Promise<void> {
  if (currentPlatform) return; // Already initialized

  currentPlatform = detectPlatform();

  if (currentPlatform === 'react-native') {
    // Dynamic import for React Native - only loads when actually running in RN
    const rn = await import('./react-native/index.js');
    storage = rn.createStorage();
    lifecycle = rn.createLifecycle();
    network = rn.createNetwork();
    deviceInfo = rn.createDeviceInfo();
  } else if (currentPlatform === 'browser') {
    // Dynamic import for browser
    const browser = await import('./browser/index.js');
    storage = browser.createStorage();
    lifecycle = browser.createLifecycle();
    network = browser.createNetwork();
    deviceInfo = browser.createDeviceInfo();
  } else {
    // Node.js
    const node = await import('./node/index.js');
    storage = node.createStorage();
    lifecycle = node.createLifecycle();
    network = node.createNetwork();
    deviceInfo = node.createDeviceInfo();
  }
}

/**
 * Get the storage adapter.
 */
export function getStorage(): StorageAdapter {
  if (!storage) {
    throw new Error('Platform not initialized. Call initializePlatform() first.');
  }
  return storage;
}

/**
 * Get the lifecycle adapter.
 */
export function getLifecycle(): LifecycleAdapter {
  if (!lifecycle) {
    throw new Error('Platform not initialized. Call initializePlatform() first.');
  }
  return lifecycle;
}

/**
 * Get the network adapter.
 */
export function getNetwork(): NetworkAdapter {
  if (!network) {
    throw new Error('Platform not initialized. Call initializePlatform() first.');
  }
  return network;
}

/**
 * Get the device info adapter.
 */
export function getDeviceInfo(): DeviceInfoAdapter {
  if (!deviceInfo) {
    throw new Error('Platform not initialized. Call initializePlatform() first.');
  }
  return deviceInfo;
}