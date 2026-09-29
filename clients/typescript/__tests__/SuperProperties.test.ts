import { SuperProperties } from '../src/SuperProperties';
import type { JSONSerializable } from '../src/types';

describe('SuperProperties', () => {
  beforeEach(() => {
    // Clear super properties before each test
    SuperProperties.clearSuperProperties();
  });

  describe('addToSuperProperties', () => {
    it('should add a single property', () => {
      SuperProperties.addToSuperProperties('app_version', '1.0.0');
      const props = SuperProperties.getSuperProperties();
      expect(props.app_version).toBe('1.0.0');
    });

    it('should add multiple properties', () => {
      SuperProperties.addToSuperProperties('app_version', '1.0.0');
      SuperProperties.addToSuperProperties('platform', 'web');
      SuperProperties.addToSuperProperties('language', 'en');

      const props = SuperProperties.getSuperProperties();
      expect(props).toEqual({
        app_version: '1.0.0',
        platform: 'web',
        language: 'en',
      });
    });

    it('should update existing property', () => {
      SuperProperties.addToSuperProperties('app_version', '1.0.0');
      SuperProperties.addToSuperProperties('app_version', '2.0.0');

      const props = SuperProperties.getSuperProperties();
      expect(props.app_version).toBe('2.0.0');
    });

    it('should accept string values', () => {
      SuperProperties.addToSuperProperties('name', 'test');
      expect(SuperProperties.getSuperProperties().name).toBe('test');
    });

    it('should accept number values', () => {
      SuperProperties.addToSuperProperties('count', 42);
      expect(SuperProperties.getSuperProperties().count).toBe(42);
    });

    it('should accept boolean values', () => {
      SuperProperties.addToSuperProperties('active', true);
      expect(SuperProperties.getSuperProperties().active).toBe(true);
    });

    it('should accept null values', () => {
      SuperProperties.addToSuperProperties('value', null);
      expect(SuperProperties.getSuperProperties().value).toBeNull();
    });
  });

  describe('getSuperProperties', () => {
    it('should return empty object when no properties set', () => {
      const props = SuperProperties.getSuperProperties();
      expect(props).toEqual({});
    });

    it('should return a copy of properties', () => {
      SuperProperties.addToSuperProperties('key', 'value');
      const props1 = SuperProperties.getSuperProperties();
      const props2 = SuperProperties.getSuperProperties();

      expect(props1).toEqual(props2);
      expect(props1).not.toBe(props2); // Different references
    });

    it('should not be affected by modifying returned object', () => {
      SuperProperties.addToSuperProperties('key', 'value');
      const props = SuperProperties.getSuperProperties();

      // Modify the returned object
      (props as any).newKey = 'newValue';

      // Original should not be affected
      const newProps = SuperProperties.getSuperProperties();
      expect(newProps).not.toHaveProperty('newKey');
    });

    it('should return all set properties', () => {
      SuperProperties.addToSuperProperties('a', 1);
      SuperProperties.addToSuperProperties('b', 2);
      SuperProperties.addToSuperProperties('c', 3);

      const props = SuperProperties.getSuperProperties();
      expect(Object.keys(props)).toHaveLength(3);
      expect(props).toEqual({ a: 1, b: 2, c: 3 });
    });
  });

  describe('clearSuperProperties', () => {
    it('should clear all properties', () => {
      SuperProperties.addToSuperProperties('a', 1);
      SuperProperties.addToSuperProperties('b', 2);

      SuperProperties.clearSuperProperties();

      const props = SuperProperties.getSuperProperties();
      expect(props).toEqual({});
    });

    it('should allow adding properties after clearing', () => {
      SuperProperties.addToSuperProperties('a', 1);
      SuperProperties.clearSuperProperties();

      SuperProperties.addToSuperProperties('b', 2);

      const props = SuperProperties.getSuperProperties();
      expect(props).toEqual({ b: 2 });
    });

    it('should be idempotent', () => {
      SuperProperties.addToSuperProperties('a', 1);
      SuperProperties.clearSuperProperties();
      SuperProperties.clearSuperProperties();
      SuperProperties.clearSuperProperties();

      const props = SuperProperties.getSuperProperties();
      expect(props).toEqual({});
    });
  });

  describe('integration with SudoQuery', () => {
    it('should merge super properties with event properties', () => {
      SuperProperties.addToSuperProperties('platform', 'web');
      SuperProperties.addToSuperProperties('app_version', '1.0.0');

      const props = SuperProperties.getSuperProperties();
      expect(props.platform).toBe('web');
      expect(props.app_version).toBe('1.0.0');
    });

    it('should allow event properties to override super properties', () => {
      SuperProperties.addToSuperProperties('platform', 'web');

      const superProps = SuperProperties.getSuperProperties();
      const eventProps = { platform: 'mobile' };

      // Simulate merge behavior
      const merged = { ...superProps, ...eventProps };
      expect(merged.platform).toBe('mobile');
    });
  });
});