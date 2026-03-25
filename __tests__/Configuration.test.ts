import { Configuration } from '../src/Configuration';

describe('Configuration', () => {
  beforeEach(() => {
    // Reset to default before each test
    Configuration.setBatchSize(10);
  });

  describe('batchSize getter', () => {
    it('should return default value of 10', () => {
      expect(Configuration.batchSize).toBe(10);
    });

    it('should return the value that was set', () => {
      Configuration.setBatchSize(20);
      expect(Configuration.batchSize).toBe(20);
    });
  });

  describe('batchSize setter', () => {
    it('should allow setting custom values', () => {
      Configuration.setBatchSize(5);
      expect(Configuration.batchSize).toBe(5);

      Configuration.setBatchSize(100);
      expect(Configuration.batchSize).toBe(100);

      Configuration.setBatchSize(1);
      expect(Configuration.batchSize).toBe(1);
    });

    it('should allow setting to zero', () => {
      Configuration.setBatchSize(0);
      expect(Configuration.batchSize).toBe(0);
    });

    it('should allow setting to negative values', () => {
      Configuration.setBatchSize(-10);
      expect(Configuration.batchSize).toBe(-10);
    });

    it('should allow setting to very large numbers', () => {
      Configuration.setBatchSize(999999);
      expect(Configuration.batchSize).toBe(999999);
    });
  });

  describe('static behavior', () => {
    it('should maintain state across different test contexts', () => {
      Configuration.setBatchSize(42);
      expect(Configuration.batchSize).toBe(42);
    });

    it('should not require instantiation', () => {
      // Should work without new Configuration()
      expect(Configuration.batchSize).toBeDefined();
    });
  });
});
