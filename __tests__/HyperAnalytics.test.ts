import { HyperAnalytics } from '../src/HyperAnalytics';

describe('HyperAnalytics', () => {
  beforeEach(() => {
    // Reset initialization state before each test
    // Note: There's no public method to reset didInit, so we can't fully isolate tests
    // This is a limitation of the current implementation
  });

  describe('init', () => {
    it('should initialize the analytics SDK', () => {
      // First call should succeed
      expect(() => HyperAnalytics.init()).not.toThrow();
    });

    it('should prevent double initialization', () => {
      HyperAnalytics.init();

      // Second call should not throw but should also not reinitialize
      expect(() => HyperAnalytics.init()).not.toThrow();
    });

    it('should be idempotent - multiple calls are safe', () => {
      expect(() => {
        HyperAnalytics.init();
        HyperAnalytics.init();
        HyperAnalytics.init();
      }).not.toThrow();
    });
  });

  describe('track', () => {
    beforeEach(() => {
      // Initialize before each track test
      HyperAnalytics.init();
    });

    describe('with valid primitive properties', () => {
      it('should not throw with string properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { key: 'value' });
        }).not.toThrow();
      });

      it('should not throw with number properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { count: 42 });
        }).not.toThrow();
      });

      it('should not throw with boolean properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { active: true });
        }).not.toThrow();
      });

      it('should not throw with null properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { value: null });
        }).not.toThrow();
      });

      it('should not throw with mixed primitive properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', {
            name: 'test',
            count: 42,
            active: true,
            value: null,
          });
        }).not.toThrow();
      });

      it('should not throw with empty object properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', {});
        }).not.toThrow();
      });

      it('should accept any event name string', () => {
        expect(() => {
          HyperAnalytics.track('page_view', {});
          HyperAnalytics.track('button_click', {});
          HyperAnalytics.track('user_signup', {});
        }).not.toThrow();
      });
    });

    describe('with invalid non-primitive properties', () => {
      it('should throw with nested object properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { user: { name: 'test' } });
        }).toThrow('only primitives are allowed as properties');
      });

      it('should throw with array properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { items: [1, 2, 3] });
        }).toThrow('only primitives are allowed as properties');
      });

      it('should throw with empty array properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { items: [] });
        }).toThrow('only primitives are allowed as properties');
      });

      it('should throw with deeply nested object properties', () => {
        expect(() => {
          HyperAnalytics.track('event_name', {
            level1: {
              level2: {
                level3: { value: 'deep' },
              },
            },
          });
        }).toThrow('only primitives are allowed as properties');
      });

      it('should throw with array of objects', () => {
        expect(() => {
          HyperAnalytics.track('event_name', {
            users: [{ name: 'a' }, { name: 'b' }],
          });
        }).toThrow('only primitives are allowed as properties');
      });

      it('should throw with mixed nested structures', () => {
        expect(() => {
          HyperAnalytics.track('event_name', {
            name: 'test',
            nested: { value: 'inner' },
          });
        }).toThrow('only primitives are allowed as properties');
      });

      it('should throw an Error object with correct message', () => {
        expect(() => {
          HyperAnalytics.track('event_name', { nested: {} });
        }).toThrow('only primitives are allowed as properties');
      });
    });

    describe('edge cases', () => {
      it('should handle special event names', () => {
        expect(() => {
          HyperAnalytics.track('', {});
          HyperAnalytics.track('event with spaces', {});
          HyperAnalytics.track('event-with-dashes', {});
          HyperAnalytics.track('event_with_underscores', {});
        }).not.toThrow();
      });

      it('should handle properties with many keys', () => {
        const manyProps: Record<string, string | number> = {};
        for (let i = 0; i < 100; i++) {
          manyProps[`key${i}`] = i;
        }

        expect(() => {
          HyperAnalytics.track('event_name', manyProps);
        }).not.toThrow();
      });

      it('should handle very long event names', () => {
        const longName = 'a'.repeat(1000);
        expect(() => {
          HyperAnalytics.track(longName, {});
        }).not.toThrow();
      });
    });
  });

  describe('static behavior', () => {
    it('should not require instantiation', () => {
      expect(() => {
        HyperAnalytics.init();
        HyperAnalytics.track('test', {});
      }).not.toThrow();
    });

    it('should maintain state across calls', () => {
      HyperAnalytics.init();
      expect(() => {
        HyperAnalytics.track('event1', {});
        HyperAnalytics.track('event2', {});
        HyperAnalytics.track('event3', {});
      }).not.toThrow();
    });
  });

  describe('user management', () => {
    beforeEach(() => {
      // Reset user state before each test
      HyperAnalytics.removeUser();
    });

    describe('setUser', () => {
      it('should set a user ID', () => {
        HyperAnalytics.setUser('user_123');
        expect(HyperAnalytics.getUser()).toBe('user_123');
      });

      it('should update existing user ID', () => {
        HyperAnalytics.setUser('user_123');
        HyperAnalytics.setUser('user_456');
        expect(HyperAnalytics.getUser()).toBe('user_456');
      });

      it('should accept empty string as user ID', () => {
        HyperAnalytics.setUser('');
        expect(HyperAnalytics.getUser()).toBe('');
      });

      it('should accept special characters in user ID', () => {
        HyperAnalytics.setUser('user@example.com');
        expect(HyperAnalytics.getUser()).toBe('user@example.com');
      });

      it('should be idempotent', () => {
        HyperAnalytics.setUser('user_123');
        HyperAnalytics.setUser('user_123');
        expect(HyperAnalytics.getUser()).toBe('user_123');
      });
    });

    describe('getUser', () => {
      it('should return null when no user is set', () => {
        expect(HyperAnalytics.getUser()).toBeNull();
      });

      it('should return the set user ID', () => {
        HyperAnalytics.setUser('user_123');
        expect(HyperAnalytics.getUser()).toBe('user_123');
      });

      it('should return the most recently set user ID', () => {
        HyperAnalytics.setUser('user_123');
        HyperAnalytics.setUser('user_456');
        expect(HyperAnalytics.getUser()).toBe('user_456');
      });
    });

    describe('removeUser', () => {
      it('should remove the current user ID', () => {
        HyperAnalytics.setUser('user_123');
        HyperAnalytics.removeUser();
        expect(HyperAnalytics.getUser()).toBeNull();
      });

      it('should be idempotent', () => {
        HyperAnalytics.setUser('user_123');
        HyperAnalytics.removeUser();
        HyperAnalytics.removeUser();
        HyperAnalytics.removeUser();
        expect(HyperAnalytics.getUser()).toBeNull();
      });

      it('should allow setting a new user after removal', () => {
        HyperAnalytics.setUser('user_123');
        HyperAnalytics.removeUser();
        HyperAnalytics.setUser('user_456');
        expect(HyperAnalytics.getUser()).toBe('user_456');
      });
    });
  });

  describe('group management', () => {
    beforeEach(() => {
      // Reset group state before each test
      HyperAnalytics.removeGroup();
    });

    describe('setGroup', () => {
      it('should set a group ID', () => {
        HyperAnalytics.setGroup('group_abc');
        expect(HyperAnalytics.getGroup()).toBe('group_abc');
      });

      it('should update existing group ID', () => {
        HyperAnalytics.setGroup('group_abc');
        HyperAnalytics.setGroup('group_xyz');
        expect(HyperAnalytics.getGroup()).toBe('group_xyz');
      });

      it('should accept empty string as group ID', () => {
        HyperAnalytics.setGroup('');
        expect(HyperAnalytics.getGroup()).toBe('');
      });

      it('should accept special characters in group ID', () => {
        HyperAnalytics.setGroup('team-alpha');
        expect(HyperAnalytics.getGroup()).toBe('team-alpha');
      });

      it('should be idempotent', () => {
        HyperAnalytics.setGroup('group_abc');
        HyperAnalytics.setGroup('group_abc');
        expect(HyperAnalytics.getGroup()).toBe('group_abc');
      });
    });

    describe('getGroup', () => {
      it('should return null when no group is set', () => {
        expect(HyperAnalytics.getGroup()).toBeNull();
      });

      it('should return the set group ID', () => {
        HyperAnalytics.setGroup('group_abc');
        expect(HyperAnalytics.getGroup()).toBe('group_abc');
      });

      it('should return the most recently set group ID', () => {
        HyperAnalytics.setGroup('group_abc');
        HyperAnalytics.setGroup('group_xyz');
        expect(HyperAnalytics.getGroup()).toBe('group_xyz');
      });
    });

    describe('removeGroup', () => {
      it('should remove the current group ID', () => {
        HyperAnalytics.setGroup('group_abc');
        HyperAnalytics.removeGroup();
        expect(HyperAnalytics.getGroup()).toBeNull();
      });

      it('should be idempotent', () => {
        HyperAnalytics.setGroup('group_abc');
        HyperAnalytics.removeGroup();
        HyperAnalytics.removeGroup();
        HyperAnalytics.removeGroup();
        expect(HyperAnalytics.getGroup()).toBeNull();
      });

      it('should allow setting a new group after removal', () => {
        HyperAnalytics.setGroup('group_abc');
        HyperAnalytics.removeGroup();
        HyperAnalytics.setGroup('group_xyz');
        expect(HyperAnalytics.getGroup()).toBe('group_xyz');
      });
    });
  });

  describe('integration with track', () => {
    beforeEach(() => {
      HyperAnalytics.init();
      HyperAnalytics.removeUser();
      HyperAnalytics.removeGroup();
    });

    it('should not throw when tracking with user ID set', () => {
      HyperAnalytics.setUser('user_123');
      expect(() => {
        HyperAnalytics.track('test_event', {});
      }).not.toThrow();
    });

    it('should not throw when tracking with group ID set', () => {
      HyperAnalytics.setGroup('group_abc');
      expect(() => {
        HyperAnalytics.track('test_event', {});
      }).not.toThrow();
    });

    it('should not throw when tracking with both user and group IDs set', () => {
      HyperAnalytics.setUser('user_123');
      HyperAnalytics.setGroup('group_abc');
      expect(() => {
        HyperAnalytics.track('test_event', {});
      }).not.toThrow();
    });

    it('should not throw when tracking without user or group', () => {
      expect(() => {
        HyperAnalytics.track('test_event', {});
      }).not.toThrow();
    });
  });
});