import { SudoQuery } from '../src/SudoQuery';
import { Batcher } from '../src/Batcher';
import { Configuration } from '../src/Configuration';

describe('SudoQuery', () => {
  beforeEach(() => {
    Configuration.reset();
    Configuration.setTenantId('tenant-1');
    Batcher.reset();
    SudoQuery['didInit'] = false;
    SudoQuery.removeUser();
  });

  describe('init', () => {
    it('should initialize the analytics SDK', () => {
      // First call should succeed
      expect(() => SudoQuery.init()).not.toThrow();
    });

    it('should prevent double initialization', () => {
      SudoQuery.init();

      // Second call should not throw but should also not reinitialize
      expect(() => SudoQuery.init()).not.toThrow();
    });

    it('should be idempotent - multiple calls are safe', () => {
      expect(() => {
        SudoQuery.init();
        SudoQuery.init();
        SudoQuery.init();
      }).not.toThrow();
    });
  });

  describe('track', () => {
    beforeEach(() => {
      // Initialize before each track test
      SudoQuery.init();
    });

    describe('with valid primitive properties', () => {
      it('should not throw with string properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { key: 'value' });
        }).not.toThrow();
      });

      it('should not throw with number properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { count: 42 });
        }).not.toThrow();
      });

      it('should not throw with boolean properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { active: true });
        }).not.toThrow();
      });

      it('should not throw with null properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { value: null });
        }).not.toThrow();
      });

      it('should not throw with mixed primitive properties', () => {
        expect(() => {
          SudoQuery.track('event_name', {
            name: 'test',
            count: 42,
            active: true,
            value: null,
          });
        }).not.toThrow();
      });

      it('should not throw with empty object properties', () => {
        expect(() => {
          SudoQuery.track('event_name', {});
        }).not.toThrow();
      });

      it('should accept any event name string', () => {
        expect(() => {
          SudoQuery.track('page_view', {});
          SudoQuery.track('button_click', {});
          SudoQuery.track('user_signup', {});
        }).not.toThrow();
      });
    });

    describe('with nested JSON properties', () => {
      it('should not throw with nested object properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { user: { name: 'test' } });
        }).not.toThrow();
      });

      it('should not throw with array properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { items: [1, 2, 3] });
        }).not.toThrow();
      });

      it('should not throw with empty array properties', () => {
        expect(() => {
          SudoQuery.track('event_name', { items: [] });
        }).not.toThrow();
      });

      it('should not throw with deeply nested object properties', () => {
        expect(() => {
          SudoQuery.track('event_name', {
            level1: {
              level2: {
                level3: { value: 'deep' },
              },
            },
          });
        }).not.toThrow();
      });

      it('should not throw with array of objects', () => {
        expect(() => {
          SudoQuery.track('event_name', {
            users: [{ name: 'a' }, { name: 'b' }],
          });
        }).not.toThrow();
      });

      it('should not throw with mixed nested structures', () => {
        expect(() => {
          SudoQuery.track('event_name', {
            name: 'test',
            nested: { value: 'inner' },
          });
        }).not.toThrow();
      });

      it('should keep nested properties in the queued collector event', () => {
        const properties = { nested: { value: 'inner' } };

        SudoQuery.track('event_name', properties);

        const batch = Batcher.fetchBatchToUpload();
        expect(batch?.[0]).toEqual(expect.objectContaining({
          name: 'event_name',
          tenant_id: 'tenant-1',
          properties,
        }));
      });
    });

    describe('without required collector configuration', () => {
      it('should throw when tenantId is missing', () => {
        Configuration.setTenantId(null);

        expect(() => {
          SudoQuery.track('event_name', {});
        }).toThrow('tenantId is required before tracking events');
      });
    });

    describe('edge cases', () => {
      it('should handle special event names', () => {
        expect(() => {
          SudoQuery.track('', {});
          SudoQuery.track('event with spaces', {});
          SudoQuery.track('event-with-dashes', {});
          SudoQuery.track('event_with_underscores', {});
        }).not.toThrow();
      });

      it('should handle properties with many keys', () => {
        const manyProps: Record<string, string | number> = {};
        for (let i = 0; i < 100; i++) {
          manyProps[`key${i}`] = i;
        }

        expect(() => {
          SudoQuery.track('event_name', manyProps);
        }).not.toThrow();
      });

      it('should handle very long event names', () => {
        const longName = 'a'.repeat(1000);
        expect(() => {
          SudoQuery.track(longName, {});
        }).not.toThrow();
      });
    });
  });

  describe('static behavior', () => {
    it('should not require instantiation', () => {
      expect(() => {
        SudoQuery.init();
        SudoQuery.track('test', {});
      }).not.toThrow();
    });

    it('should maintain state across calls', () => {
      SudoQuery.init();
      expect(() => {
        SudoQuery.track('event1', {});
        SudoQuery.track('event2', {});
        SudoQuery.track('event3', {});
      }).not.toThrow();
    });
  });

  describe('user management', () => {
    beforeEach(() => {
      // Reset user state before each test
      SudoQuery.removeUser();
    });

    describe('setUser', () => {
      it('should set a user ID', () => {
        SudoQuery.setUser('user_123');
        expect(SudoQuery.getUser()).toBe('user_123');
      });

      it('should update existing user ID', () => {
        SudoQuery.setUser('user_123');
        SudoQuery.setUser('user_456');
        expect(SudoQuery.getUser()).toBe('user_456');
      });

      it('should accept empty string as user ID', () => {
        SudoQuery.setUser('');
        expect(SudoQuery.getUser()).toBe('');
      });

      it('should accept special characters in user ID', () => {
        SudoQuery.setUser('user@example.com');
        expect(SudoQuery.getUser()).toBe('user@example.com');
      });

      it('should be idempotent', () => {
        SudoQuery.setUser('user_123');
        SudoQuery.setUser('user_123');
        expect(SudoQuery.getUser()).toBe('user_123');
      });
    });

    describe('getUser', () => {
      it('should return null when no user is set', () => {
        expect(SudoQuery.getUser()).toBeNull();
      });

      it('should return the set user ID', () => {
        SudoQuery.setUser('user_123');
        expect(SudoQuery.getUser()).toBe('user_123');
      });

      it('should return the most recently set user ID', () => {
        SudoQuery.setUser('user_123');
        SudoQuery.setUser('user_456');
        expect(SudoQuery.getUser()).toBe('user_456');
      });
    });

    describe('removeUser', () => {
      it('should remove the current user ID', () => {
        SudoQuery.setUser('user_123');
        SudoQuery.removeUser();
        expect(SudoQuery.getUser()).toBeNull();
      });

      it('should be idempotent', () => {
        SudoQuery.setUser('user_123');
        SudoQuery.removeUser();
        SudoQuery.removeUser();
        SudoQuery.removeUser();
        expect(SudoQuery.getUser()).toBeNull();
      });

      it('should allow setting a new user after removal', () => {
        SudoQuery.setUser('user_123');
        SudoQuery.removeUser();
        SudoQuery.setUser('user_456');
        expect(SudoQuery.getUser()).toBe('user_456');
      });
    });
  });

  describe('integration with track', () => {
    beforeEach(() => {
      SudoQuery.init();
      SudoQuery.removeUser();
    });

    it('should not throw when tracking with user ID set', () => {
      SudoQuery.setUser('user_123');
      expect(() => {
        SudoQuery.track('test_event', {});
      }).not.toThrow();
    });

    it('should not throw when tracking without user', () => {
      expect(() => {
        SudoQuery.track('test_event', {});
      }).not.toThrow();
    });
  });
});
