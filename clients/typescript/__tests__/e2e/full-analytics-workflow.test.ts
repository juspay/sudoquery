import { SudoQuery } from '../../src/SudoQuery';
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
  beforeEach(() => {
    // Reset all state before each test
    Configuration.reset();
    Configuration.setBatchSize(3);
    Configuration.setOrgId('acme-org-1');
    Configuration.setProjectId('acme-project-1');
    SudoQuery['didInit'] = false;
    Batcher.reset();
    jest.clearAllMocks();
  });

  describe('complete user journey', () => {
    it('should handle the full analytics lifecycle: init -> track -> batch', async () => {
      // Step 1: Initialize analytics
      SudoQuery.init();
      expect(SudoQuery['didInit']).toBe(true);

      // Step 2: Track multiple events with valid properties
      SudoQuery.track('page_view', { page: '/home' });
      SudoQuery.track('button_click', { button: 'submit' });
      SudoQuery.track('form_submit', { form: 'contact' });

      // Step 3: Verify batching using public API
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).not.toBeNull();
      expect(batch).toHaveLength(3);
    });

    it('should handle tracking events with nested properties', () => {
      // Step 1: Initialize
      SudoQuery.init();

      // Step 2: Track valid event
      SudoQuery.track('valid_event', { prop: 'value' });

      // Step 3: Track a nested payload accepted by the collector
      expect(() => {
        SudoQuery.track('nested_event', { nested: { value: 'test' } });
      }).not.toThrow();

      // Step 4: Verify both events were batched
      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });

  describe('configuration changes', () => {
    it('should handle configuration changes affecting runtime behavior', () => {
      SudoQuery.init();

      // Initial configuration
      Configuration.setBatchSize(2);

      SudoQuery.track('event_1', { id: 1 });
      SudoQuery.track('event_2', { id: 2 });

      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);

      Batcher.setMarkLastBatchUploaded();

      // Change configuration
      Configuration.setBatchSize(3);

      SudoQuery.track('event_3', { id: 3 });
      SudoQuery.track('event_4', { id: 4 });

      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });

  describe('error recovery', () => {
    it('should recover from initialization errors', () => {
      // First initialization
      SudoQuery.init();
      expect(SudoQuery['didInit']).toBe(true);

      // Second initialization (should be idempotent)
      SudoQuery.init();
      expect(SudoQuery['didInit']).toBe(true);

      // Should still work
      SudoQuery.track('event_1', { id: 1 });

      const batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(1);
    });
  });

  describe('state persistence across operations', () => {
    it('should maintain state across multiple init-track cycles', async () => {
      // Cycle 1
      SudoQuery.init();
      Configuration.setBatchSize(2);

      SudoQuery.track('event_1', { id: 1 });
      SudoQuery.track('event_2', { id: 2 });

      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);

      // Cycle 2
      Batcher.reset();
      SudoQuery.track('event_3', { id: 3 });
      SudoQuery.track('event_4', { id: 4 });

      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });

  describe('real-world scenarios', () => {
    it('should simulate a typical web application analytics flow', async () => {
      SudoQuery.init();
      Configuration.setBatchSize(3);

      // User lands on page
      SudoQuery.track('page_view', { page: '/home', referrer: 'google' });

      // User interacts with page
      SudoQuery.track('click', { element: 'button', action: 'signup' });

      // User fills form
      SudoQuery.track('form_start', { form: 'signup' });
      SudoQuery.track('form_submit', { form: 'signup', success: true });

      // User navigates
      SudoQuery.track('page_view', { page: '/dashboard' });

      // Verify batches
      let batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(3);

      Batcher.setMarkLastBatchUploaded();
      batch = Batcher.fetchBatchToUpload();
      expect(batch).toHaveLength(2);
    });
  });
});
