import type { BatchPayload, Event, JSONSerializable, SystemProperties } from '../../src/types';

describe('Type Tests', () => {
  describe('JSONSerializable type', () => {
    it('should accept primitive, array, and nested object values', () => {
      const value: JSONSerializable = {
        name: 'test',
        count: 42,
        active: true,
        missing: null,
        tags: ['a', 'b'],
        nested: {
          value: 'deep',
        },
      };

      expect(value).toBeDefined();
    });

    it('should accept large JSON-compatible objects', () => {
      const largeObj: Record<string, number> = {};
      for (let i = 0; i < 1000; i++) {
        largeObj[`key${i}`] = i;
      }

      const value: JSONSerializable = largeObj;
      expect(Object.keys(value)).toHaveLength(1000);
    });
  });

  describe('collector event type', () => {
    const systemProperties: SystemProperties = {
      geo: {
        country: 'IN',
      },
      timezone: 'Asia/Kolkata',
    };

    it('should accept the collector event envelope', () => {
      const event: Event = {
        envelop_version: '1.0',
        id: '018f6a31-6f27-7c9c-8e21-cfc10f5ee879',
        name: 'page_view',
        org_id: 'tenant-1',
        proj_id: 'workspace-1',
        session_id: 'session-1',
        anon_id: 'anon-1',
        actor_id: 'user-1',
        source: 'typescript',
        occured_at: '2026-08-25T12:00:00.000Z',
        properties: {
          page: '/home',
          metadata: {
            referrer: 'direct',
          },
        },
        correlation_id: null,
        trace_id: null,
        system_properties: systemProperties,
      };

      expect(event.name).toBe('page_view');
      expect(event.actor_id).toBe('user-1');
      expect(event.properties).toEqual({
        page: '/home',
        metadata: {
          referrer: 'direct',
        },
      });
    });

    it('should allow nullable optional collector fields', () => {
      const event: Event = {
        envelop_version: '1.0',
        id: '018f6a31-6f27-7c9c-8e21-cfc10f5ee879',
        name: 'anonymous_event',
        org_id: 'tenant-1',
        proj_id: null,
        session_id: null,
        anon_id: 'anon-1',
        actor_id: null,
        source: null,
        occured_at: '2026-08-25T12:00:00.000Z',
        properties: null,
        correlation_id: null,
        trace_id: null,
        system_properties: null,
      };

      expect(event.proj_id).toBeNull();
      expect(event.actor_id).toBeNull();
    });
  });

  describe('batch payload type', () => {
    it('should accept collector batch payloads', () => {
      const event: Event = {
        envelop_version: '1.0',
        id: '018f6a31-6f27-7c9c-8e21-cfc10f5ee879',
        name: 'purchase',
        org_id: 'tenant-1',
        proj_id: null,
        session_id: 'session-1',
        anon_id: 'anon-1',
        actor_id: 'user-1',
        source: 'typescript',
        occured_at: '2026-08-25T12:00:00.000Z',
        properties: {
          amount: 99.99,
          currency: 'USD',
        },
        correlation_id: null,
        trace_id: null,
        system_properties: null,
      };

      const payload: BatchPayload = {
        events: [event],
        system_properties: {
          geo: null,
          timezone: 'Asia/Kolkata',
        },
      };

      expect(payload.events).toHaveLength(1);
      expect(payload.system_properties?.timezone).toBe('Asia/Kolkata');
    });
  });
});
