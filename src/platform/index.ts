/**
 * Platform abstraction layer.
 * Detects runtime environment and provides appropriate adapters.
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

// Import adapters statically to avoid dynamic import issues
import { createStorage as createBrowserStorage, createLifecycle as createBrowserLifecycle, createNetwork as createBrowserNetwork, createDeviceInfo as createBrowserDeviceInfo } from './browser/index';
import { createStorage as createNodeStorage, createLifecycle as createNodeLifecycle, createNetwork as createNodeNetwork, createDeviceInfo as createNodeDeviceInfo } from './node/index';
import { createStorage as createRNStorage, createLifecycle as createRNLifecycle, createNetwork as createRNNetwork, createDeviceInfo as createRNDeviceInfo } from './react-native/index';

/**
 * Initialize the platform abstraction layer.
 * Must be called before using any adapters.
 */
export async function initializePlatform(): Promise<void> {
  if (currentPlatform) return; // Already initialized

  currentPlatform = detectPlatform();

  if (currentPlatform === 'react-native') {
    storage = createRNStorage();
    lifecycle = createRNLifecycle();
    network = createRNNetwork();
    deviceInfo = createRNDeviceInfo();
  } else if (currentPlatform === 'browser') {
    storage = createBrowserStorage();
    lifecycle = createBrowserLifecycle();
    network = createBrowserNetwork();
    deviceInfo = createBrowserDeviceInfo();
  } else {
    // Node.js
    storage = createNodeStorage();
    lifecycle = createNodeLifecycle();
    network = createNodeNetwork();
    deviceInfo = createNodeDeviceInfo();
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