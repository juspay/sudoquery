import { Batcher } from '../../src/Batcher';
import { Pusher } from '../../src/Pusher';
import { flush } from '../../src/Flush';
import { Configuration } from '../../src/Configuration';

// Mock Pusher
jest.mock('../../src/Pusher', () => ({
  Pusher: {
    _isUploadInProgress: false,
    pushLogs: jest.fn().mockResolvedValue(null),
    startScheduler: jest.fn(),
  },
}));

describe('Batcher and Flush Integration', () => {
  beforeEach(() => {
    // Reset state before each test
    Configuration.setBatchSize(3);
    Batcher.reset();
    jest.clearAllMocks();
  });

  const createMockEvent = (id: number) => ({
    eventName: `event_${id}`,
    eventId: crypto.randomUUID(),
    properties: { id },
    user: `user_${id}`,
    anon_id: crypto.randomUUID(),
    at: Date.now(),
  });

  describe('batch lifecycle', () => {
    it('should create and fill batches correctly', () => {
      // Create batches
      Configuration.setBatchSize(2);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2)); // First batch complete
      Batcher.addToBatch(createMockEvent(3));
      Batcher.addToBatch(createMockEvent(4)); // Second batch complete

      // Verify batches using public API
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toBeNull();
    });

    it('should handle partial batches', () => {
      Configuration.setBatchSize(5);

      // Add only 3 events (partial batch)
      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));
      Batcher.addToBatch(createMockEvent(3));

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(3);
    });

    it('should handle empty batches', () => {
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toBeNull();
    });
  });

  describe('concurrent batch creation', () => {
    it('should handle adding events while batching', () => {
      Configuration.setBatchSize(2);

      // Add initial events
      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));

      // Fetch and mark
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();

      // Add more events
      Batcher.addToBatch(createMockEvent(3));
      Batcher.addToBatch(createMockEvent(4));

      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);
    });

    it('should handle multiple batches', () => {
      Configuration.setBatchSize(2);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));
      Batcher.addToBatch(createMockEvent(3));
      Batcher.addToBatch(createMockEvent(4));

      // Verify 2 batches
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toBeNull();
    });
  });

  describe('batch state management', () => {
    it('should correctly track uploaded batches', () => {
      Configuration.setBatchSize(2);

      // Create 3 batches
      for (let i = 1; i <= 6; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      // Verify all batches using public API
      for (let i = 0; i < 3; i++) {
        const batch = Batcher.fetchBatchToUpload();
        expect(batch).toHaveLength(2);
        Batcher.setMarkLastBatchUploaded();
      }

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toBeNull();
    });

    it('should resume from last uploaded batch', () => {
      Configuration.setBatchSize(2);

      // Create 4 batches
      for (let i = 1; i <= 8; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      // First fetch - upload 2 batches
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();

      // Second fetch - should continue from batch 2
      const batch3 = Batcher.fetchBatchToUpload();
      expect(batch3).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch4 = Batcher.fetchBatchToUpload();
      expect(batch4).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();
      const batch5 = Batcher.fetchBatchToUpload();
      expect(batch5).toBeNull();
    });
  });

  describe('batch size configuration', () => {
    it('should adapt to changing batch sizes', () => {
      Configuration.setBatchSize(2);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2)); // Batch 0 complete

      // Fetch the batch (this triggers creation of new accumulating batch)
      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();

      // Change batch size
      Configuration.setBatchSize(5);

      // Now add new events - they go to the new accumulating batch
      Batcher.addToBatch(createMockEvent(3));
      Batcher.addToBatch(createMockEvent(4));
      Batcher.addToBatch(createMockEvent(5));
      Batcher.addToBatch(createMockEvent(6));
      Batcher.addToBatch(createMockEvent(7)); // New batch complete with new size

      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(5);
    });

    it('should handle large batch sizes', () => {
      Configuration.setBatchSize(50);

      // Add 75 events
      for (let i = 1; i <= 75; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const batch1 = Batcher.fetchBatchToUpload();
      expect(batch1).toHaveLength(50);

      Batcher.setMarkLastBatchUploaded();
      const batch2 = Batcher.fetchBatchToUpload();
      expect(batch2).toHaveLength(25);
    });

    it('should handle batch size of 1', () => {
      Configuration.setBatchSize(1);

      // Add 5 events
      for (let i = 1; i <= 5; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      // Verify 5 batches
      for (let i = 0; i < 5; i++) {
        const batch = Batcher.fetchBatchToUpload();
        expect(batch).toHaveLength(1);
        Batcher.setMarkLastBatchUploaded();
      }
    });
  });

  describe('edge cases', () => {
    it('should handle no batches available', () => {
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toBeNull();
    });

    it('should handle single batch', () => {
      Configuration.setBatchSize(3);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));
      Batcher.addToBatch(createMockEvent(3));

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(3);
    });

    it('should handle events with varying sizes', () => {
      Configuration.setBatchSize(2);

      const smallEvent = createMockEvent(1);
      const largeEvent = {
        ...createMockEvent(2),
        properties: {
          ...Array.from({ length: 100 }, (_, i) => [`key${i}`, `value${i}`]).reduce((acc, [k, v]) => ({ ...acc, [k]: v }), {}),
        },
      };

      Batcher.addToBatch(smallEvent);
      Batcher.addToBatch(largeEvent);

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });
});
