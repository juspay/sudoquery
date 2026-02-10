import { add, logMessage, containsNonPrimitives } from '../src/index';
import type { JSONSerializable } from '../src/types';

describe('index.ts exports', () => {
  describe('add function', () => {
    it('should add two positive numbers', () => {
      expect(add(2, 3)).toBe(5);
      expect(add(10, 20)).toBe(30);
      expect(add(100, 200)).toBe(300);
    });

    it('should add two negative numbers', () => {
      expect(add(-2, -3)).toBe(-5);
      expect(add(-10, -20)).toBe(-30);
    });

    it('should add positive and negative numbers', () => {
      expect(add(10, -5)).toBe(5);
      expect(add(-10, 5)).toBe(-5);
      expect(add(100, -50)).toBe(50);
    });

    it('should add zero', () => {
      expect(add(0, 0)).toBe(0);
      expect(add(5, 0)).toBe(5);
      expect(add(0, 5)).toBe(5);
      expect(add(-5, 0)).toBe(-5);
    });

    it('should handle decimal numbers', () => {
      expect(add(1.5, 2.5)).toBe(4);
      expect(add(0.1, 0.2)).toBeCloseTo(0.3);
      expect(add(-1.5, 2.5)).toBe(1);
    });

    it('should handle very large numbers', () => {
      expect(add(1000000, 2000000)).toBe(3000000);
      expect(add(Number.MAX_SAFE_INTEGER, 1)).toBe(Number.MAX_SAFE_INTEGER + 1);
    });

    it('should handle very small numbers', () => {
      expect(add(0.0001, 0.0002)).toBeCloseTo(0.0003);
    });
  });

  describe('logMessage function', () => {
    beforeEach(() => {
      jest.spyOn(console, 'log').mockImplementation();
    });

    afterEach(() => {
      jest.restoreAllMocks();
    });

    it('should log message with prefix', () => {
      logMessage('test message');

      expect(console.log).toHaveBeenCalledWith('[MyLib]: test message');
    });

    it('should log empty string', () => {
      logMessage('');

      expect(console.log).toHaveBeenCalledWith('[MyLib]: ');
    });

    it('should log message with special characters', () => {
      logMessage('test!@#$%^&*()');

      expect(console.log).toHaveBeenCalledWith('[MyLib]: test!@#$%^&*()');
    });

    it('should log message with emojis', () => {
      logMessage('test 😊');

      expect(console.log).toHaveBeenCalledWith('[MyLib]: test 😊');
    });

    it('should log very long messages', () => {
      const longMessage = 'a'.repeat(1000);
      logMessage(longMessage);

      expect(console.log).toHaveBeenCalledWith(`[MyLib]: ${longMessage}`);
    });

    it('should log multiple messages', () => {
      logMessage('message 1');
      logMessage('message 2');
      logMessage('message 3');

      expect(console.log).toHaveBeenCalledTimes(3);
      expect(console.log).toHaveBeenNthCalledWith(1, '[MyLib]: message 1');
      expect(console.log).toHaveBeenNthCalledWith(2, '[MyLib]: message 2');
      expect(console.log).toHaveBeenNthCalledWith(3, '[MyLib]: message 3');
    });

    it('should not throw errors', () => {
      expect(() => {
        logMessage('test');
      }).not.toThrow();
    });
  });

  describe('containsNonPrimitives function', () => {
    it('should be exported from index', () => {
      expect(containsNonPrimitives).toBeDefined();
      expect(typeof containsNonPrimitives).toBe('function');
    });

    it('should return false for primitives only', () => {
      const obj = { name: 'test', count: 42 };
      expect(containsNonPrimitives(obj)).toBe(false);
    });

    it('should return true for nested objects', () => {
      const obj = { user: { name: 'test' } };
      expect(containsNonPrimitives(obj)).toBe(true);
    });

    it('should return true for arrays', () => {
      const obj = { items: [1, 2, 3] };
      expect(containsNonPrimitives(obj)).toBe(true);
    });
  });

  describe('JSONSerializable type', () => {
    it('should be exported from index', () => {
      // Type check - this will fail at compile time if not exported
      const value: JSONSerializable = { test: 'value' };
      expect(value).toBeDefined();
    });

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

  describe('module exports', () => {
    it('should export all expected functions', () => {
      expect(typeof add).toBe('function');
      expect(typeof logMessage).toBe('function');
      expect(typeof containsNonPrimitives).toBe('function');
    });

    it('should export all expected types', () => {
      // Type exports are checked at compile time
      // This test ensures the module structure is correct
      const module = require('../src/index');
      expect(module).toHaveProperty('add');
      expect(module).toHaveProperty('logMessage');
      expect(module).toHaveProperty('containsNonPrimitives');
    });
  });

  describe('integration between exported functions', () => {
    it('should work together without conflicts', () => {
      const sum = add(5, 10);
      expect(sum).toBe(15);

      jest.spyOn(console, 'log').mockImplementation();
      logMessage(`Sum is ${sum}`);
      expect(console.log).toHaveBeenCalled();
      jest.restoreAllMocks();
    });

    it('should maintain independent behavior', () => {
      const result1 = add(1, 2);
      const result2 = add(3, 4);

      expect(result1).toBe(3);
      expect(result2).toBe(7);

      const hasPrimitives1 = containsNonPrimitives({ a: 1 });
      const hasPrimitives2 = containsNonPrimitives({ a: { b: 1 } });

      expect(hasPrimitives1).toBe(false);
      expect(hasPrimitives2).toBe(true);
    });
  });
});