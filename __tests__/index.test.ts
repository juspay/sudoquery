import {
  HyperAnalytics,
  detectPlatform,
  isPlatformInitialized,
  type JSONSerializable,
  type Event,
  type SessionData,
  type BatchPayload,
  type ClientEvent,
} from '../src/index';

describe('index.ts exports', () => {
  describe('HyperAnalytics', () => {
    it('should export HyperAnalytics class', () => {
      expect(HyperAnalytics).toBeDefined();
      expect(typeof HyperAnalytics).toBe('function');
    });

    it('should have static methods', () => {
      expect(typeof HyperAnalytics.init).toBe('function');
      expect(typeof HyperAnalytics.track).toBe('function');
      expect(typeof HyperAnalytics.setUser).toBe('function');
      expect(typeof HyperAnalytics.getUser).toBe('function');
      expect(typeof HyperAnalytics.removeUser).toBe('function');
      expect(typeof HyperAnalytics.flush).toBe('function');
      expect(typeof HyperAnalytics.setSuperProperty).toBe('function');
      expect(typeof HyperAnalytics.getSuperProperties).toBe('function');
      expect(typeof HyperAnalytics.clearSuperProperties).toBe('function');
    });

    it('should have static getters', () => {
      expect(typeof HyperAnalytics.batchSize).toBe('number');
      expect(typeof HyperAnalytics.endpoint).toBe('string');
    });
  });

  describe('platform utilities', () => {
    it('should export detectPlatform function', () => {
      expect(detectPlatform).toBeDefined();
      expect(typeof detectPlatform).toBe('function');
    });

    it('should export isPlatformInitialized function', () => {
      expect(isPlatformInitialized).toBeDefined();
      expect(typeof isPlatformInitialized).toBe('function');
    });

    it('should return valid platform type', () => {
      const platform = detectPlatform();
      expect(['browser', 'react-native', 'node']).toContain(platform);
    });
  });

  describe('JSONSerializable type', () => {
    it('should accept primitive values', () => {
      const str: JSONSerializable = 'test';
      const num: JSONSerializable = 42;
      const bool: JSONSerializable = true;
      const nullValue: JSONSerializable = null;

      expect(str).toBe('test');
      expect(num).toBe(42);
      expect(bool).toBe(true);
      expect(nullValue).toBe(null);
    });

    it('should accept arrays', () => {
      const arr: JSONSerializable = [1, 2, 3];
      expect(arr).toEqual([1, 2, 3]);
    });

    it('should accept objects', () => {
      const obj: JSONSerializable = { key: 'value' };
      expect(obj).toEqual({ key: 'value' });
    });

    it('should accept nested structures', () => {
      const nested: JSONSerializable = {
        level1: {
          level2: [1, 2, 3],
        },
      };
      expect(nested).toBeDefined();
    });
  });

  describe('Event type', () => {
    it('should be exported and usable', () => {
      const event: Event = {
        eventName: 'test',
        properties: { foo: 'bar' },
        user: 'user123',
        anon_id: 'anon123',
        eventId: 'event123',
        at: Date.now(),
      };
      expect(event).toBeDefined();
      expect(event.eventName).toBe('test');
    });
  });

  describe('SessionData type', () => {
    it('should be exported and usable', () => {
      const session: SessionData = {
        device_type: 'desktop',
        platform: 'Windows',
        browser: 'Chrome',
        country: '',
        city: '',
        ip_address: null,
        user_agent: 'Mozilla/5.0',
      };
      expect(session).toBeDefined();
      expect(session.device_type).toBe('desktop');
    });
  });

  describe('BatchPayload type', () => {
    it('should be exported and usable', () => {
      const payload: BatchPayload = {
        session: {
          device_type: 'desktop',
          platform: 'Windows',
          browser: 'Chrome',
          country: '',
          city: '',
          ip_address: null,
          user_agent: 'Mozilla/5.0',
        },
        events: [],
      };
      expect(payload).toBeDefined();
      expect(payload.events).toEqual([]);
    });
  });

  describe('ClientEvent type', () => {
    it('should be exported and usable', () => {
      const clientEvent: ClientEvent = {
        event_id: '123',
        event_name: 'test',
        event_timestamp: Date.now(),
        user_id: 'user123',
        anon_id: 'anon123',
        properties: '{}',
      };
      expect(clientEvent).toBeDefined();
      expect(clientEvent.event_name).toBe('test');
    });
  });

  describe('module exports', () => {
    it('should export all expected functions and classes', () => {
      const module = require('../src/index');
      expect(module.HyperAnalytics).toBeDefined();
      expect(module.detectPlatform).toBeDefined();
      expect(module.isPlatformInitialized).toBeDefined();
    });
  });
});
