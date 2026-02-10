import type { JSONSerializable, Event } from '../../src/types';

describe('Type Tests', () => {
  describe('JSONSerializable type', () => {
    it('should accept string values', () => {
      const value: JSONSerializable = 'test string';
      expect(value).toBe('test string');
    });

    it('should accept number values', () => {
      const value: JSONSerializable = 42;
      expect(value).toBe(42);
    });

    it('should accept decimal numbers', () => {
      const value: JSONSerializable = 3.14;
      expect(value).toBe(3.14);
    });

    it('should accept boolean values', () => {
      const value1: JSONSerializable = true;
      const value2: JSONSerializable = false;
      expect(value1).toBe(true);
      expect(value2).toBe(false);
    });

    it('should accept null values', () => {
      const value: JSONSerializable = null;
      expect(value).toBeNull();
    });

    it('should accept arrays of primitives', () => {
      const value: JSONSerializable = [1, 2, 3];
      expect(value).toEqual([1, 2, 3]);
    });

    it('should accept arrays of strings', () => {
      const value: JSONSerializable = ['a', 'b', 'c'];
      expect(value).toEqual(['a', 'b', 'c']);
    });

    it('should accept arrays of mixed primitives', () => {
      const value: JSONSerializable = [1, 'two', true, null];
      expect(value).toEqual([1, 'two', true, null]);
    });

    it('should accept empty arrays', () => {
      const value: JSONSerializable = [];
      expect(value).toEqual([]);
    });

    it('should accept objects with primitive values', () => {
      const value: JSONSerializable = { key: 'value', count: 42 };
      expect(value).toEqual({ key: 'value', count: 42 });
    });

    it('should accept objects with multiple primitive types', () => {
      const value: JSONSerializable = {
        name: 'test',
        age: 25,
        active: true,
        value: null,
      };
      expect(value).toEqual({
        name: 'test',
        age: 25,
        active: true,
        value: null,
      });
    });

    it('should accept empty objects', () => {
      const value: JSONSerializable = {};
      expect(value).toEqual({});
    });

    it('should accept nested objects', () => {
      const value: JSONSerializable = {
        level1: {
          level2: {
            level3: {
              value: 'deep',
            },
          },
        },
      };
      expect(value).toBeDefined();
    });

    it('should accept arrays of objects', () => {
      const value: JSONSerializable = [
        { id: 1, name: 'a' },
        { id: 2, name: 'b' },
      ];
      expect(value).toEqual([
        { id: 1, name: 'a' },
        { id: 2, name: 'b' },
      ]);
    });

    it('should accept objects with array properties', () => {
      const value: JSONSerializable = {
        items: [1, 2, 3],
        tags: ['a', 'b', 'c'],
      };
      expect(value).toEqual({
        items: [1, 2, 3],
        tags: ['a', 'b', 'c'],
      });
    });

    it('should accept complex nested structures', () => {
      const value: JSONSerializable = {
        users: [
          {
            id: 1,
            name: 'Alice',
            posts: [
              { id: 101, title: 'Post 1' },
              { id: 102, title: 'Post 2' },
            ],
          },
          {
            id: 2,
            name: 'Bob',
            posts: [],
          },
        ],
        metadata: {
          total: 2,
          active: true,
        },
      };
      expect(value).toBeDefined();
    });

    it('should accept very large objects', () => {
      const largeObj: Record<string, number> = {};
      for (let i = 0; i < 1000; i++) {
        largeObj[`key${i}`] = i;
      }
      const value: JSONSerializable = largeObj;
      expect(Object.keys(value)).toHaveLength(1000);
    });

    it('should accept very large arrays', () => {
      const largeArr: number[] = [];
      for (let i = 0; i < 1000; i++) {
        largeArr.push(i);
      }
      const value: JSONSerializable = largeArr;
      expect(value).toHaveLength(1000);
    });
  });

  describe('Event type', () => {
    it('should accept valid event objects', () => {
      const event: Event = {
        eventName: 'page_view',
        properties: { page: '/home' },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(event).toBeDefined();
    });

    it('should accept events with string properties', () => {
      const event: Event = {
        eventName: 'button_click',
        properties: { button: 'submit', color: 'blue' },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect((event.properties as Record<string, unknown>).button).toBe('submit');
    });

    it('should accept events with number properties', () => {
      const event: Event = {
        eventName: 'scroll',
        properties: { depth: 50, speed: 2.5 },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect((event.properties as Record<string, unknown>).depth).toBe(50);
    });

    it('should accept events with boolean properties', () => {
      const event: Event = {
        eventName: 'form_submit',
        properties: { success: true, validated: false },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect((event.properties as Record<string, unknown>).success).toBe(true);
    });

    it('should accept events with null properties', () => {
      const event: Event = {
        eventName: 'error',
        properties: { message: null, code: null },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect((event.properties as Record<string, unknown>).message).toBeNull();
    });

    it('should accept events with mixed primitive properties', () => {
      const event: Event = {
        eventName: 'purchase',
        properties: {
          product_id: 'prod_123',
          quantity: 2,
          price: 99.99,
          in_stock: true,
          discount: null,
        },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect((event.properties as Record<string, unknown>).quantity).toBe(2);
    });

    it('should accept events with empty properties object', () => {
      const event: Event = {
        eventName: 'simple_event',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(Object.keys(event.properties as Record<string, unknown>)).toHaveLength(0);
    });

    it('should accept events with many properties', () => {
      const properties: Record<string, string | number> = {};
      for (let i = 0; i < 100; i++) {
        properties[`prop${i}`] = i;
      }

      const event: Event = {
        eventName: 'complex_event',
        properties,
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(Object.keys(event.properties as Record<string, unknown>)).toHaveLength(100);
    });

    it('should accept events with various user and group identifiers', () => {
      const event1: Event = {
        eventName: 'event_1',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      const event2: Event = {
        eventName: 'event_2',
        properties: {},
        user: 'anonymous_user',
        group: 'public',
        anon_id: 'anon_456',
        eventId: 'evt_456',
        at: Date.now(),
      };

      expect(event1.user).toBe('user_123');
      expect(event2.group).toBe('public');
    });

    it('should accept events with timestamp as number', () => {
      const timestamp = Date.now();
      const event: Event = {
        eventName: 'timestamp_test',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: timestamp,
      };
      expect(event.at).toBe(timestamp);
    });

    it('should accept events with timestamp 0', () => {
      const event: Event = {
        eventName: 'epoch_test',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: 0,
      };
      expect(event.at).toBe(0);
    });

    it('should accept events with large timestamps', () => {
      const futureTimestamp = 9999999999999;
      const event: Event = {
        eventName: 'future_test',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: futureTimestamp,
      };
      expect(event.at).toBe(futureTimestamp);
    });
  });

  describe('type compatibility', () => {
    it('should allow JSONSerializable as Event properties', () => {
      const properties: JSONSerializable = {
        key: 'value',
        count: 42,
      };

      const event: Event = {
        eventName: 'test_event',
        properties,
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      expect(event.properties).toEqual(properties);
    });

    it('should allow assigning Event properties to JSONSerializable', () => {
      const event: Event = {
        eventName: 'test_event',
        properties: { key: 'value' },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      const props: JSONSerializable = event.properties;
      expect(props).toEqual({ key: 'value' });
    });

    it('should allow arrays of Events', () => {
      const events: Event[] = [
        {
          eventName: 'event_1',
          properties: { id: 1 },
          user: 'user_123',
          group: 'group_abc',
          anon_id: 'anon_123',
          eventId: 'evt_123',
          at: Date.now(),
        },
        {
          eventName: 'event_2',
          properties: { id: 2 },
          user: 'user_123',
          group: 'group_abc',
          anon_id: 'anon_456',
          eventId: 'evt_456',
          at: Date.now(),
        },
      ];

      expect(events).toHaveLength(2);
    });

    it('should allow arrays of Events as JSONSerializable', () => {
      const events: Event[] = [
        {
          eventName: 'event_1',
          properties: { id: 1 },
          user: 'user_123',
          group: 'group_abc',
          anon_id: 'anon_123',
          eventId: 'evt_123',
          at: Date.now(),
        },
      ];

      const value: JSONSerializable = events;
      expect(value).toHaveLength(1);
    });
  });

  describe('type safety', () => {
    it('should enforce string type for event name', () => {
      const event: Event = {
        eventName: 'test_event',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(typeof event.eventName).toBe('string');
    });

    it('should enforce string type for user', () => {
      const event: Event = {
        eventName: 'test_event',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(typeof event.user).toBe('string');
    });

    it('should enforce string type for group', () => {
      const event: Event = {
        eventName: 'test_event',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(typeof event.group).toBe('string');
    });

    it('should enforce number type for timestamp', () => {
      const event: Event = {
        eventName: 'test_event',
        properties: {},
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };
      expect(typeof event.at).toBe('number');
    });

    it('should enforce JSONSerializable type for properties', () => {
      const event: Event = {
        eventName: 'test_event',
        properties: { key: 'value' },
        user: 'user_123',
        group: 'group_abc',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      const props: JSONSerializable = event.properties;
      expect(props).toBeDefined();
    });
  });

  describe('real-world type scenarios', () => {
    it('should handle e-commerce event types', () => {
      const productView: Event = {
        eventName: 'product_view',
        properties: {
          product_id: 'prod_123',
          category: 'electronics',
          price: 99.99,
          in_stock: true,
        },
        user: 'user_123',
        group: 'customers',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      const purchase: Event = {
        eventName: 'purchase',
        properties: {
          order_id: 'order_456',
          total: 199.98,
          items: 2,
          payment_method: 'credit_card',
        },
        user: 'user_123',
        group: 'customers',
        anon_id: 'anon_456',
        eventId: 'evt_456',
        at: Date.now(),
      };

      expect(productView.eventName).toBe('product_view');
      expect((purchase.properties as Record<string, unknown>).total).toBe(199.98);
    });

    it('should handle analytics event types', () => {
      const pageView: Event = {
        eventName: 'page_view',
        properties: {
          page: '/home',
          referrer: 'google',
          load_time: 1.5,
        },
        user: 'user_123',
        group: 'visitors',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      const click: Event = {
        eventName: 'click',
        properties: {
          element: 'button',
          action: 'submit',
          x: 100,
          y: 200,
        },
        user: 'user_123',
        group: 'visitors',
        anon_id: 'anon_456',
        eventId: 'evt_456',
        at: Date.now(),
      };

      expect((pageView.properties as Record<string, unknown>).page).toBe('/home');
      expect((click.properties as Record<string, unknown>).element).toBe('button');
    });

    it('should handle user behavior event types', () => {
      const scroll: Event = {
        eventName: 'scroll',
        properties: {
          depth: 75,
          direction: 'down',
          speed: 2.5,
        },
        user: 'user_123',
        group: 'engaged_users',
        anon_id: 'anon_123',
        eventId: 'evt_123',
        at: Date.now(),
      };

      const formSubmit: Event = {
        eventName: 'form_submit',
        properties: {
          form_id: 'contact_form',
          success: true,
          validation_errors: null,
        },
        user: 'user_123',
        group: 'engaged_users',
        anon_id: 'anon_456',
        eventId: 'evt_456',
        at: Date.now(),
      };

      expect((scroll.properties as Record<string, unknown>).depth).toBe(75);
      expect((formSubmit.properties as Record<string, unknown>).success).toBe(true);
    });
  });
});