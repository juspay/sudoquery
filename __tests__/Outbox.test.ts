import { Outbox, OutboxLocks, OutboxStorage } from '../src/Outbox';
import type { Event } from '../src/types';
import { createMockEvent } from './testUtils';

const NOW = 1_700_000_000_000;

class MemoryStorage implements OutboxStorage {
  private items = new Map<string, string>();
  get length() {
    return this.items.size;
  }
  key(index: number) {
    return [...this.items.keys()][index] ?? null;
  }
  getItem(key: string) {
    return this.items.get(key) ?? null;
  }
  setItem(key: string, value: string) {
    this.items.set(key, value);
  }
  removeItem(key: string) {
    this.items.delete(key);
  }
  outboxEvents(): Event[] {
    return [...this.items.entries()]
      .filter(([key]) => key.startsWith('sudoquery_outbox:'))
      .flatMap(([, value]) => JSON.parse(value));
  }
}

/** Grants every request immediately and tracks held locks until their callback settles. */
class MemoryLocks implements OutboxLocks {
  held = new Set<string>();
  async request(name: string, callback: () => Promise<unknown>) {
    this.held.add(name);
    try {
      return await callback();
    } finally {
      this.held.delete(name);
    }
  }
  async query() {
    return { held: [...this.held].map((name) => ({ name })) };
  }
}

function events(count: number, start = 1): Event[] {
  return Array.from({ length: count }, (_, i) => createMockEvent(start + i));
}

describe('Outbox', () => {
  let storage: MemoryStorage;
  let locks: MemoryLocks;
  const newOutbox = (overrides = {}) =>
    new Outbox({ storage, locks, isOffline: () => false, now: () => NOW, ...overrides });

  beforeEach(() => {
    storage = new MemoryStorage();
    locks = new MemoryLocks();
  });

  describe('storing', () => {
    it('should store a batch that failed to send', () => {
      const outbox = newOutbox();
      const batch = events(3);

      outbox.onFailed(batch);

      expect(storage.outboxEvents()).toEqual(batch);
    });

    it('should remove stored events once they are delivered', () => {
      const outbox = newOutbox();
      const batch = events(3);
      outbox.onFailed(batch);

      outbox.onDelivered(batch);

      expect(storage.outboxEvents()).toEqual([]);
      expect(storage.length).toBe(0);
    });

    it('should not store unload sends on a healthy network', () => {
      const outbox = newOutbox();

      outbox.onUnloadSend([events(2)]);

      expect(storage.length).toBe(0);
    });

    it('should store unload sends after a failure', () => {
      const outbox = newOutbox();
      outbox.onFailed(events(1, 1));

      outbox.onUnloadSend([events(2, 2), events(1, 4)]);

      expect(storage.outboxEvents().map((e) => e.name)).toEqual([
        'event_1',
        'event_2',
        'event_3',
        'event_4',
      ]);
    });

    it('should store unload sends while the browser is offline', () => {
      const outbox = newOutbox({ isOffline: () => true });

      outbox.onUnloadSend([events(2)]);

      expect(storage.outboxEvents()).toHaveLength(2);
    });

    it('should stop treating unload sends as failing after a delivery', () => {
      const outbox = newOutbox();
      outbox.onFailed(events(1, 1));
      outbox.onDelivered(events(1, 2));

      outbox.onUnloadSend([events(1, 3)]);

      expect(storage.outboxEvents().map((e) => e.name)).toEqual(['event_1']);
    });

    it('should not store the same event twice', () => {
      const outbox = newOutbox();
      const batch = events(2);

      outbox.onFailed(batch);
      outbox.onFailed(batch);

      expect(storage.outboxEvents()).toHaveLength(2);
    });

    it('should keep only the newest events over the cap', () => {
      const outbox = newOutbox({ maxEvents: 3 });

      outbox.onFailed(events(5));

      expect(storage.outboxEvents().map((e) => e.name)).toEqual(['event_3', 'event_4', 'event_5']);
    });

    it('should ignore storage errors', () => {
      const outbox = newOutbox();
      storage.setItem = () => {
        throw new Error('QuotaExceededError');
      };

      expect(() => outbox.onFailed(events(2))).not.toThrow();
    });

    it('should do nothing without storage', async () => {
      const outbox = new Outbox({ storage: null, locks: null });

      expect(() => outbox.onFailed(events(2))).not.toThrow();
      expect(await outbox.start()).toEqual([]);
    });
  });

  describe('restoring', () => {
    it('should return events left by a previous page load and clear them', async () => {
      const previous = newOutbox();
      previous.onFailed(events(3));

      const current = newOutbox();
      const restored = await current.start();

      expect(restored.map((e) => e.name)).toEqual(['event_1', 'event_2', 'event_3']);
      expect(storage.length).toBe(0);
    });

    it('should not claim events from a page that is still open', async () => {
      const openTab = newOutbox();
      await openTab.start(); // holds its owner lock
      openTab.onFailed(events(2));

      const newTab = newOutbox();
      const restored = await newTab.start();

      expect(restored).toEqual([]);
      expect(storage.outboxEvents()).toHaveLength(2);
    });

    it('should claim every other page load without the Web Locks API', async () => {
      const openTab = newOutbox({ locks: null });
      await openTab.start();
      openTab.onFailed(events(2));

      const restored = await newOutbox({ locks: null }).start();

      expect(restored).toHaveLength(2);
    });

    it('should merge, dedupe and order events from several page loads', async () => {
      const shared = events(4);
      newOutbox().onFailed([shared[2], shared[0]]);
      newOutbox().onFailed([shared[3], shared[0], shared[1]]);

      const restored = await newOutbox().start();

      expect(restored.map((e) => e.name)).toEqual(['event_1', 'event_2', 'event_3', 'event_4']);
    });

    it('should drop expired events', async () => {
      const day = 24 * 60 * 60 * 1000;
      newOutbox({ maxAgeMs: Infinity }).onFailed([
        createMockEvent(1, { occured_at: new Date(NOW - 8 * day).toISOString() }),
        createMockEvent(2, { occured_at: new Date(NOW - 1 * day).toISOString() }),
      ]);

      const restored = await newOutbox().start();

      expect(restored.map((e) => e.name)).toEqual(['event_2']);
    });

    it('should skip corrupt storage entries', async () => {
      storage.setItem('sudoquery_outbox:broken', '{not json');
      newOutbox().onFailed(events(1));

      const restored = await newOutbox().start();

      expect(restored).toHaveLength(1);
    });
  });
});
