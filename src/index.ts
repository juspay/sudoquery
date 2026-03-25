// src/index.ts
export { HyperAnalytics, type HyperAnalyticsConfig } from './HyperAnalytics';
export type { JSONSerializable, Event, SessionData, BatchPayload, ClientEvent } from './types';

// Export platform utilities for advanced users
export {
  detectPlatform,
  getPlatform,
  isPlatformInitialized,
  resetPlatform,
  type StorageAdapter,
  type LifecycleAdapter,
  type NetworkAdapter,
  type DeviceInfoAdapter,
  type PlatformType,
} from './platform';
