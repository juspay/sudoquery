import { Batcher } from '../src/Batcher';
import { Configuration } from '../src/Configuration';
import type { Event } from '../src/types';
import { createMockEvent } from './testUtils';

// Mock Flush module to prevent auto-flush during tests
jest.mock('../src/Flush', () => ({
  flush: jest.fn().mockResolvedValue(undefined),
}));

describe('Batcher', () => {
  beforeEach(() => {
    // Reset configuration and batcher state before each test
    Configuration.setBatchSize(10);
    Batcher.reset();
  });

  describe('addToBatch', () => {
    it('should add events to batches', () => {
      const event = createMockEvent(1);
      Batcher.addToBatch(event);

      // Verify we can fetch a batch
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).not.toBeNull();
      expect(batch).toContainEqual(event);
    });

    it('should add multiple events to the same batch', () => {
      const event1 = createMockEvent(1);
      const event2 = createMockEvent(2);
      const event3 = createMockEvent(3);

      Batcher.addToBatch(event1);
      Batcher.addToBatch(event2);
      Batcher.addToBatch(event3);

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(3);
      expect(batch).toContainEqual(event1);
      expect(batch).toContainEqual(event2);
      expect(batch).toContainEqual(event3);
    });

    it('should create a new batch when current batch reaches batch size', () => {
      Configuration.setBatchSize(3);

      const event1 = createMockEvent(1);
      const event2 = createMockEvent(2);
      const event3 = createMockEvent(3);
      const event4 = createMockEvent(4);

      Batcher.addToBatch(event1);
      Batcher.addToBatch(event2);
      Batcher.addToBatch(event3);
      Batcher.addToBatch(event4);

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(3);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(1);
      expect(batch2).toContainEqual(event4);
    });

    it('should respect custom batch size', () => {
      Configuration.setBatchSize(5);

      for (let i = 1; i <= 7; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(5);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);
    });

    it('should handle batch size of 1', () => {
      Configuration.setBatchSize(1);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(1);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(1);
    });

    it('should handle large batch sizes', () => {
      Configuration.setBatchSize(100);

      for (let i = 1; i <= 150; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(100);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(50);
    });
  });

  describe('fetchBatchToUpload', () => {
    it('should return the first batch when called initially', () => {
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 5; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).not.toBeNull();
      expect(batch).toHaveLength(3);
    });

    it('should return null when no batches are available', () => {
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toBeNull();
    });

    it('should return subsequent batches on consecutive calls', () => {
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 5; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(3);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toBeNull();
    });
  });

  describe('setMarkLastBatchUploaded', () => {
    it('should allow marking batch as uploaded', () => {
      Configuration.setBatchSize(2);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();

      // Next call should not return the same batch
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toBeNull();
    });

    it('should work correctly with multiple calls', () => {
      Configuration.setBatchSize(2);

      // Add 6 events (3 batches)
      for (let i = 1; i <= 6; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      // Fetch and mark all batches
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);
      Batcher.setMarkLastBatchUploaded();

      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);
      Batcher.setMarkLastBatchUploaded();

      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toHaveLength(2);
      Batcher.setMarkLastBatchUploaded();

      const batch4 = Batcher.fetchBatchToUpload();
      expect(batch4).toBeNull();
    });
  });

  describe('integration with Configuration', () => {
    it('should use updated batch size after configuration change', () => {
      Configuration.setBatchSize(2);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));
      Batcher.addToBatch(createMockEvent(3));

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(1);

      // Mark batch2 as uploaded (creates new empty batch)
      Batcher.setMarkLastBatchUploaded();

      // Change batch size
      Configuration.setBatchSize(5);

      Batcher.addToBatch(createMockEvent(4));
      Batcher.addToBatch(createMockEvent(5));

      // Current batch should now have 2 events (since new batch was created)
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toHaveLength(2);
    });
  });

  describe('static behavior', () => {
    it('should not require instantiation', () => {
      expect(() => {
        Batcher.addToBatch(createMockEvent(1));
        Batcher.fetchBatchToUpload();
        Batcher.setMarkLastBatchUploaded();
      }).not.toThrow();
    });

    it('should maintain state across calls', () => {
      const event1 = createMockEvent(1);
      const event2 = createMockEvent(2);

      Batcher.addToBatch(event1);
      const batch1 = Batcher.fetchBatchToUpload();

      Batcher.setMarkLastBatchUploaded();

      Batcher.addToBatch(event2);
      const batch2 = Batcher.fetchBatchToUpload();

      expect(batch1).not.toBeNull();
      expect(batch2).not.toBeNull();
    });
  });

  describe('edge cases', () => {
    it('should handle events with large payloads', () => {
      Configuration.setBatchSize(2);

      const largeEvent: Event = createMockEvent(1, {
        name: 'large_event',
        properties: {
          ...Array.from({ length: 100 }, (_, i) => [`key${i}`, `value${i}`]).reduce((acc, [k, v]) => ({ ...acc, [k]: v }), {}),
        },
      });

      Batcher.addToBatch(largeEvent);
      Batcher.addToBatch(largeEvent);

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });
});
