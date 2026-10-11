import { useSyncExternalStore } from 'react';
// The SDK source in clients/typescript (aliased in vite.config.ts), not the
// published sudo-query package that src/utils/analytics.ts uses. They're
// separate module instances, so the demo's collector settings leave the
// dashboard's own analytics alone.
import { SudoQuery, type JSONSerializable } from '@clients/typescript';
import { DEMO_CONFIG } from '../config/api';
import type { Destination } from './destination';

type EventProperties = Record<string, JSONSerializable>;

export type TrackedEvent = {
  key: number;
  name: string;
  properties: EventProperties;
  at: Date;
};

const MAX_TRACKED_EVENTS = 50;

let trackedEvents: TrackedEvent[] = [];
let nextKey = 0;
const listeners = new Set<() => void>();

/**
 * Takes effect once per page load: the SDK keeps its first configuration, so
 * changing the destination means reloading the page.
 */
export function initDemoAnalytics({ tenantId, workspaceId }: Destination): void {
  SudoQuery.init({
    endpoint: DEMO_CONFIG.COLLECTOR_URL,
    tenantId,
    workspaceId,
    source: 'dashboard-demo',
    // Small batches on a short timer, so events reach the collector while
    // you're still clicking around.
    batchSize: 5,
    flushInterval: 3000,
  });
}

export function trackDemoEvent(name: string, properties: EventProperties): void {
  SudoQuery.track(name, properties);

  trackedEvents = [
    { key: nextKey++, name, properties, at: new Date() },
    ...trackedEvents,
  ].slice(0, MAX_TRACKED_EVENTS);
  listeners.forEach((listener) => listener());
}

export function identifyDemoCustomer(customerId: string): void {
  SudoQuery.setUser(customerId);
}

export function flushDemoEvents(): Promise<void> {
  return SudoQuery.flush();
}

/** Events tracked on this page load, newest first. */
export function useTrackedEvents(): TrackedEvent[] {
  return useSyncExternalStore(subscribe, () => trackedEvents);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
