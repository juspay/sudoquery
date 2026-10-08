/**
 * Outbox keeps undelivered events in browser storage so they are sent on a
 * later page load instead of being lost when the page closes.
 *
 * - Only events known to be failing are stored: a send failed, or the browser
 *   is offline when the page is hidden. On a healthy network nothing is written.
 * - Each page load writes to its own storage key, so tabs never overwrite each other.
 * - On start, keys left behind by page loads that are gone are claimed and their
 *   events returned for sending. With the Web Locks API a key is only claimed once
 *   its page no longer holds its owner lock, and one tab claims at a time. Without
 *   it, every other key is claimed, so a tab that is still open may have its stored
 *   events sent twice.
 * - Stored events are capped and expire.
 *
 * It depends only on the Event type: observe deliveries through its
 * onDelivered/onFailed/onUnloadSend methods and queue what start() returns.
 */

import type { Event } from "./types";
import { generateUuid } from "./Uuid";

const KEY_PREFIX = "sudoquery_outbox:";
const OWNER_LOCK_PREFIX = "sudoquery_outbox_owner:";
const CLAIM_LOCK = "sudoquery_outbox_claim";

const DEFAULT_MAX_EVENTS = 1000;
const DEFAULT_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

export type OutboxStorage = Pick<Storage, "getItem" | "setItem" | "removeItem" | "key" | "length">;

export interface OutboxLocks {
  request(name: string, callback: () => Promise<unknown>): Promise<unknown>;
  query(): Promise<{ held?: Array<{ name?: string }> }>;
}

export interface OutboxOptions {
  storage?: OutboxStorage | null;
  locks?: OutboxLocks | null;
  isOffline?: () => boolean;
  maxEvents?: number;
  maxAgeMs?: number;
  now?: () => number;
}

export class Outbox {
  private readonly ownerId = generateUuid();
  private readonly key = KEY_PREFIX + this.ownerId;
  private readonly storage: OutboxStorage | null;
  private readonly locks: OutboxLocks | null;
  private readonly isOffline: () => boolean;
  private readonly maxEvents: number;
  private readonly maxAgeMs: number;
  private readonly now: () => number;
  private failing = false;
  private hasStored = false;

  constructor(options: OutboxOptions = {}) {
    this.storage = options.storage !== undefined ? options.storage : defaultStorage();
    this.locks = options.locks !== undefined ? options.locks : defaultLocks();
    this.isOffline = options.isOffline ?? defaultIsOffline;
    this.maxEvents = options.maxEvents ?? DEFAULT_MAX_EVENTS;
    this.maxAgeMs = options.maxAgeMs ?? DEFAULT_MAX_AGE_MS;
    this.now = options.now ?? Date.now;
  }

  /**
   * Take ownership of this page's storage key and return events left behind by
   * earlier page loads. Returned events are removed from storage; they are stored
   * again if sending them fails.
   */
  async start(): Promise<Event[]> {
    if (!this.storage) return [];

    try {
      const locks = this.locks;
      if (!locks) return this.claim(new Set());

      await this.holdOwnerLock(locks);
      return (await locks.request(CLAIM_LOCK, async () => this.claim(await liveOwners(locks)))) as Event[];
    } catch (error) {
      console.error("Outbox restore failed:", error);
      return [];
    }
  }

  onFailed(batch: Event[]): void {
    this.failing = true;
    this.save(batch);
  }

  onDelivered(batch: Event[]): void {
    this.failing = false;
    if (!this.hasStored) return;

    const ids = new Set(batch.map((event) => event.id));
    const stored = this.read(this.key);
    const kept = stored.filter((event) => !ids.has(event.id));
    if (kept.length !== stored.length) this.write(kept);
  }

  onUnloadSend(batches: Event[][]): void {
    // A keepalive send's result usually arrives after the page is gone, so store
    // the events up front, but only when they are likely to fail.
    if (this.failing || this.isOffline()) {
      this.save(batches.flat());
    }
  }

  private holdOwnerLock(locks: OutboxLocks): Promise<void> {
    return new Promise((resolve) => {
      locks
        .request(OWNER_LOCK_PREFIX + this.ownerId, () => {
          resolve();
          // Never settles, so the lock is held until the page goes away.
          return new Promise(() => {});
        })
        .catch(() => resolve());
    });
  }

  private claim(liveOwnerIds: Set<string>): Event[] {
    const keys = this.outboxKeys().filter(
      (key) => key !== this.key && !liveOwnerIds.has(key.slice(KEY_PREFIX.length)),
    );

    const events: Event[] = [];
    for (const key of keys) {
      events.push(...this.read(key));
      try {
        this.storage?.removeItem(key);
      } catch (_) {
        // Ignore: the events are already in memory.
      }
    }

    return this.prune(events);
  }

  private save(events: Event[]): void {
    if (!this.storage || events.length === 0) return;
    this.write(this.prune([...this.read(this.key), ...events]));
  }

  /**
   * Drop duplicates and expired events, order by time, and keep the newest maxEvents.
   */
  private prune(events: Event[]): Event[] {
    const oldest = this.now() - this.maxAgeMs;
    const seen = new Set<string>();
    const kept: Array<{ event: Event; time: number }> = [];

    for (const event of events) {
      const time = Date.parse(event.occured_at);
      if (seen.has(event.id) || !(time >= oldest)) continue;
      seen.add(event.id);
      kept.push({ event, time });
    }

    kept.sort((a, b) => a.time - b.time);
    return kept.slice(-this.maxEvents).map(({ event }) => event);
  }

  private outboxKeys(): string[] {
    const keys: string[] = [];
    try {
      for (let i = 0; i < (this.storage?.length ?? 0); i++) {
        const key = this.storage?.key(i);
        if (key?.startsWith(KEY_PREFIX)) keys.push(key);
      }
    } catch (_) {
      // Storage became unavailable.
    }
    return keys;
  }

  private read(key: string): Event[] {
    try {
      const parsed = JSON.parse(this.storage?.getItem(key) ?? "[]");
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  private write(events: Event[]): void {
    try {
      if (events.length === 0) {
        this.storage?.removeItem(this.key);
      } else {
        this.storage?.setItem(this.key, JSON.stringify(events));
      }
      this.hasStored = events.length > 0;
    } catch (_) {
      // Quota exceeded or storage unavailable: the events stay in memory only.
    }
  }
}

async function liveOwners(locks: OutboxLocks): Promise<Set<string>> {
  const snapshot = await locks.query();
  const ids = new Set<string>();
  for (const lock of snapshot.held ?? []) {
    if (lock.name?.startsWith(OWNER_LOCK_PREFIX)) {
      ids.add(lock.name.slice(OWNER_LOCK_PREFIX.length));
    }
  }
  return ids;
}

function defaultStorage(): OutboxStorage | null {
  try {
    return typeof window !== "undefined" && window.localStorage ? window.localStorage : null;
  } catch (_) {
    // Accessing localStorage throws in sandboxed iframes.
    return null;
  }
}

function defaultLocks(): OutboxLocks | null {
  return typeof navigator !== "undefined" && navigator.locks ? navigator.locks : null;
}

function defaultIsOffline(): boolean {
  return typeof navigator !== "undefined" && navigator.onLine === false;
}
