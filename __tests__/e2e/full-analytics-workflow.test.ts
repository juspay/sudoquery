import { HyperAnalytics } from '../../src/HyperAnalytics';
import { Batcher } from '../../src/Batcher';
import { Pusher } from '../../src/Pusher';
import { flush } from '../../src/Flush';
import { Configuration } from '../../src/Configuration';

// Mock Pusher to avoid actual network calls
jest.mock('../../src/Pusher', () => ({
  Pusher: {
    _isUploadInProgress: false,
    pushLogs: jest.fn().mockResolvedValue(null),
    startScheduler: jest.fn(),
  },
}));

describe('Full Analytics Workflow - End-to-End', () => {
  beforeEach(async () => {
    // Reset all state before each test
    Configuration.setBatchSize(3);
    await HyperAnalytics.reset();
    Batcher.reset();
    jest.clearAllMocks();
  });

  describe('complete user journey', () => {
    it('should handle the full analytics lifecycle: init -> track -> batch', async () => {
      // Step 1: Initialize analytics
      await HyperAnalytics.init();
      expect(HyperAnalytics.isInitialized).toBe(true);

      // Step 2: Track multiple events with valid properties
      HyperAnalytics.track('page_view', { page: '/home' });
      HyperAnalytics.track('button_click', { button: 'submit' });
      HyperAnalytics.track('form_submit', { form: 'contact' });

      // Step 3: Verify batching using public API
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).not.toBeNull();
      expect(batch).toHaveLength(3);
    });

    it('should handle tracking events with invalid properties', async () => {
      // Step 1: Initialize
      await HyperAnalytics.init();

      // Step 2: Track valid event
      HyperAnalytics.track('valid_event', { prop: 'value' });

      // Step 3: Attempt to track invalid event (should throw)
      expect(() => {
        HyperAnalytics.track('invalid_event', { nested: { value: 'test' } });
      }).toThrow('only primitives are allowed as properties');

      // Step 4: Verify only valid event was batched
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(1);
    });
  });

  describe('configuration changes', () => {
    it('should handle configuration changes affecting runtime behavior', async () => {
      await HyperAnalytics.init();

      // Initial configuration
      Configuration.setBatchSize(2);

      HyperAnalytics.track('event_1', { id: 1 });
      HyperAnalytics.track('event_2', { id: 2 });

      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();

      // Change configuration
      Configuration.setBatchSize(3);

      HyperAnalytics.track('event_3', { id: 3 });
      HyperAnalytics.track('event_4', { id: 4 });

      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });

  describe('error recovery', () => {
    it('should recover from initialization errors', async () => {
      // First initialization
      await HyperAnalytics.init();
      expect(HyperAnalytics.isInitialized).toBe(true);

      // Second initialization (should be idempotent)
      await HyperAnalytics.init();
      expect(HyperAnalytics.isInitialized).toBe(true);

      // Should still work
      HyperAnalytics.track('event_1', { id: 1 });

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(1);
    });
  });

  describe('state persistence across operations', () => {
    it('should maintain state across multiple init-track cycles', async () => {
      // Cycle 1
      await HyperAnalytics.init();
      Configuration.setBatchSize(2);

      HyperAnalytics.track('event_1', { id: 1 });
      HyperAnalytics.track('event_2', { id: 2 });

      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);

      // Cycle 2
      Batcher.reset();
      HyperAnalytics.track('event_3', { id: 3 });
      HyperAnalytics.track('event_4', { id: 4 });

      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });

  describe('real-world scenarios', () => {
    it('should simulate a typical web application analytics flow', async () => {
      await HyperAnalytics.init();
      Configuration.setBatchSize(3);

      // User lands on page
      HyperAnalytics.track('page_view', { page: '/home', referrer: 'google' });

      // User interacts with page
      HyperAnalytics.track('click', { element: 'button', action: 'signup' });

      // User fills form
      HyperAnalytics.track('form_start', { form: 'signup' });
      HyperAnalytics.track('form_submit', { form: 'signup', success: true });

      // User navigates
      HyperAnalytics.track('page_view', { page: '/dashboard' });

      // Verify batches
      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(3);

      Batcher.setMarkLastBatchUploaded();
      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });
});
