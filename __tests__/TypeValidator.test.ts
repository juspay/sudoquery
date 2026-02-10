import { containsNonPrimitives } from '../src/TypeValidator';

describe('containsNonPrimitives', () => {
  describe('with primitives only', () => {
    it('should return false for object with only string values', () => {
      const obj = { name: 'test', city: 'nyc' };
      expect(containsNonPrimitives(obj)).toBe(false);
    });

    it('should return false for object with only number values', () => {
      const obj = { count: 42, price: 99.99 };
      expect(containsNonPrimitives(obj)).toBe(false);
    });

    it('should return false for object with only boolean values', () => {
      const obj = { active: true, verified: false };
      expect(containsNonPrimitives(obj)).toBe(false);
    });

    it('should return false for object with null values', () => {
      const obj = { value: null, data: null };
      expect(containsNonPrimitives(obj)).toBe(false);
    });

    it('should return false for object with mixed primitive values', () => {
      const obj = {
        name: 'test',
        count: 42,
        active: true,
        value: null,
      };
      expect(containsNonPrimitives(obj)).toBe(false);
    });

    it('should return false for empty object', () => {
      const obj = {};
      expect(containsNonPrimitives(obj)).toBe(false);
    });
  });

  describe('with nested objects', () => {
    it('should return true for object with nested object', () => {
      const obj = { user: { name: 'test' } };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with multiple nested objects', () => {
      const obj = {
        user: { name: 'test' },
        settings: { theme: 'dark' },
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for deeply nested objects', () => {
      const obj = {
        level1: {
          level2: {
            level3: {
              value: 'deep',
            },
          },
        },
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with empty nested object', () => {
      const obj = { nested: {} };
      expect(containsNonPrimitives(obj)).toBe(true);
    });
  });

  describe('with arrays', () => {
    it('should return true for object with array of primitives', () => {
      const obj = { items: [1, 2, 3] };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with array of strings', () => {
      const obj = { tags: ['a', 'b', 'c'] };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with empty array', () => {
      const obj = { items: [] };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with array containing null', () => {
      const obj = { items: [null, null] };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with array of objects', () => {
      const obj = { users: [{ name: 'a' }, { name: 'b' }] };
      expect(containsNonPrimitives(obj)).toBe(true);
    });
  });

  describe('with mixed data structures', () => {
    it('should return true for object with primitives and nested object', () => {
      const obj = {
        name: 'test',
        count: 42,
        nested: { value: 'inner' },
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with primitives and array', () => {
      const obj = {
        name: 'test',
        items: [1, 2, 3],
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with nested objects and arrays', () => {
      const obj = {
        user: { name: 'test' },
        tags: ['a', 'b'],
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });
  });

  describe('edge cases', () => {
    it('should return false for null', () => {
      expect(containsNonPrimitives(null)).toBe(false);
    });

    it('should return false for undefined', () => {
      expect(containsNonPrimitives(undefined as any)).toBe(false);
    });

    it('should return false for string', () => {
      expect(containsNonPrimitives('string')).toBe(false);
    });

    it('should return false for number', () => {
      expect(containsNonPrimitives(42)).toBe(false);
    });

    it('should return false for boolean', () => {
      expect(containsNonPrimitives(true)).toBe(false);
    });

    it('should return false for array at top level', () => {
      expect(containsNonPrimitives([1, 2, 3])).toBe(false);
    });

    it('should return false for empty array at top level', () => {
      expect(containsNonPrimitives([])).toBe(false);
    });

    it('should return false for array of objects at top level', () => {
      expect(containsNonPrimitives([{ a: 1 }])).toBe(false);
    });

    it('should return false for array of arrays at top level', () => {
      expect(containsNonPrimitives([[1, 2], [3, 4]])).toBe(false);
    });
  });

  describe('complex nested structures', () => {
    it('should return true for object with array of objects', () => {
      const obj = {
        users: [
          { name: 'Alice', age: 30 },
          { name: 'Bob', age: 25 },
        ],
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with nested arrays', () => {
      const obj = {
        matrix: [[1, 2], [3, 4]],
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for object with mixed nested structures', () => {
      const obj = {
        data: {
          items: [1, 2, 3],
          meta: { count: 3 },
        },
      };
      expect(containsNonPrimitives(obj)).toBe(true);
    });
  });
});