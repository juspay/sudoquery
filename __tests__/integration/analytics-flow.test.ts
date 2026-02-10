import { HyperAnalytics } from '../../src/HyperAnalytics';
import { Batcher } from '../../src/Batcher';
import { Pusher } from '../../src/Pusher';
import { flush } from '../../src/Flush';
import { Configuration } from '../../src/Configuration';

describe('Analytics Flow Integration', () => {
  beforeEach(() => {
    // Reset all state before each test
    Configuration.batchSize = 3;
    HyperAnalytics['didInit'] = false;
    jest.clearAllMocks();
    Batcher.reset();
  });

  describe('complete analytics workflow', () => {
    it('should track events, batch them, and prepare for upload', async () => {
      // Step 1: Initialize analytics
      HyperAnalytics.init();
      expect(HyperAnalytics['didInit']).toBe(true);

      // Step 2: Track multiple events
      HyperAnalytics.track('event_1', { prop1: 'value1' });
      HyperAnalytics.track('event_2', { prop2: 'value2' });
      HyperAnalytics.track('event_3', { prop3: 'value3' });

      // Step 3: Verify batching using public API
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).not.toBeNull();
      expect(batch).toHaveLength(3);
    });

    it('should handle multiple batches correctly', () => {
      HyperAnalytics.init();

      Configuration.batchSize = 2;

      // Track 5 events (should create 3 batches: 2, 2, 1)
      for (let i = 1; i <= 5; i++) {
        HyperAnalytics.track(`event_${i}`, { id: i });
      }

      // Verify batches using public API
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toHaveLength(1);

      Batcher.setMarkLastBatchUploaded();
      const batch4 = Batcher.fetchBatchToUpload();
      expect(batch4).toBeNull();
    });

    it('should reject events with non-primitive properties', () => {
      HyperAnalytics.init();

      expect(() => {
        HyperAnalytics.track('event_1', { nested: { value: 'test' } });
      }).toThrow('only primitives are allowed as properties');

      // Should not be added to batch
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toBeNull();
    });
  });

  describe('batching behavior', () => {
    it('should respect batch size configuration', () => {
      HyperAnalytics.init();

      Configuration.batchSize = 5;

      for (let i = 1; i <= 12; i++) {
        HyperAnalytics.track(`event_${i}`, { id: i });
      }

      // Verify batches using public API
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(5);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(5);

      Batcher.setMarkLastBatchUploaded();
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toHaveLength(2);
    });

    it('should dynamically adjust to batch size changes', () => {
      HyperAnalytics.init();

      Configuration.batchSize = 2;

      HyperAnalytics.track('event_1', { id: 1 });
      HyperAnalytics.track('event_2', { id: 2 });
      HyperAnalytics.track('event_3', { id: 3 });

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(1);

      // Mark batch2 as uploaded (creates new empty batch)
      Batcher.setMarkLastBatchUploaded();

      // Change batch size mid-stream
      Configuration.batchSize = 5;

      HyperAnalytics.track('event_4', { id: 4 });
      HyperAnalytics.track('event_5', { id: 5 });

      // Current batch should now have 2 events (since new batch was created)
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toHaveLength(2);
    });

    it('should handle batch size of 1', () => {
      HyperAnalytics.init();

      Configuration.batchSize = 1;

      for (let i = 1; i <= 5; i++) {
        HyperAnalytics.track(`event_${i}`, { id: i });
      }

      // Verify batches using public API
      for (let i = 0; i < 5; i++) {
        const batch = Batcher.fetchBatchToUpload();
        expect(batch).toHaveLength(1);
        Batcher.setMarkLastBatchUploaded();
      }
    });
  });

  describe('state persistence', () => {
    it('should maintain state across multiple operations', () => {
      HyperAnalytics.init();

      // First batch
      Configuration.batchSize = 2;
      HyperAnalytics.track('event_1', { id: 1 });
      HyperAnalytics.track('event_2', { id: 2 });

      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);

      // Add more events
      HyperAnalytics.track('event_3', { id: 3 });
      HyperAnalytics.track('event_4', { id: 4 });

      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });

    it('should handle reinitialization gracefully', () => {
      HyperAnalytics.init();
      HyperAnalytics.track('event_1', { id: 1 });

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(1);

      // Reinitialize (should be idempotent)
      HyperAnalytics.init();
      expect(HyperAnalytics['didInit']).toBe(true);

      // Reset Batcher for clean test
      Batcher.reset();

      // Should still be able to track events
      HyperAnalytics.track('event_2', { id: 2 });

      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(1);
    });
  });

  describe('error handling', () => {
    it('should handle invalid event properties without crashing', () => {
      HyperAnalytics.init();

      // Valid event
      HyperAnalytics.track('valid_event', { prop: 'value' });

      // Invalid event - should throw but not crash the system
      expect(() => {
        HyperAnalytics.track('invalid_event', { nested: {} });
      }).toThrow();

      // System should still work for valid events
      expect(() => {
        HyperAnalytics.track('another_valid_event', { prop: 'value' });
      }).not.toThrow();

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });

  describe('performance and scalability', () => {
    it('should handle large volumes of events efficiently', () => {
      HyperAnalytics.init();
      Configuration.batchSize = 50;

      // Track 100 events (reduced from 1000 to avoid memory issues)
      for (let i = 1; i <= 100; i++) {
        HyperAnalytics.track(`event_${i}`, { id: i });
      }

      // Verify batches using public API
      for (let i = 0; i < 2; i++) {
        const batch = Batcher.fetchBatchToUpload();
        expect(batch).toHaveLength(50);
        Batcher.setMarkLastBatchUploaded();
      }
    });

    it('should handle events with large payloads', () => {
      HyperAnalytics.init();
      Configuration.batchSize = 2;

      const largePayload: Record<string, string> = {};
      for (let i = 0; i < 100; i++) {
        largePayload[`key${i}`] = `value${i}`;
      }

      HyperAnalytics.track('large_event', largePayload);
      HyperAnalytics.track('another_large_event', largePayload);

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });
});