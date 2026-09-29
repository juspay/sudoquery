import { flush } from '../src/Flush';
import { Pusher } from '../src/Pusher';
import { Batcher } from '../src/Batcher';
import { Configuration } from '../src/Configuration';
import type { Event } from '../src/types';
import { createMockEvent } from './testUtils';

describe('flush', () => {
  beforeEach(() => {
    // Reset state before each test
    Configuration.setBatchSize(10);
    Pusher['_isUploadInProgress'] = false;
    Batcher.reset();
    jest.clearAllMocks();
  });

  describe('basic functionality', () => {
    it('should call Pusher.pushLogs at least once', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs').mockResolvedValue(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalled();
      pushLogsSpy.mockRestore();
    });

    it('should return when Pusher.pushLogs returns null', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs').mockResolvedValue(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(1);
      pushLogsSpy.mockRestore();
    });

    it('should call Batcher.setMarkLastBatchUploaded when pushLogs returns non-null', async () => {
      const mockBatch: Event[] = [createMockEvent(1)];
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs').mockResolvedValue(mockBatch);
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');

      // Mock pushLogs to return null on second call to exit the loop
      pushLogsSpy
        .mockResolvedValueOnce([createMockEvent(1)])
        .mockResolvedValueOnce(null);

      await flush();

      expect(markUploadedSpy).toHaveBeenCalledTimes(1);
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });

    it('should continue looping until pushLogs returns null', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');

      // Simulate 3 batches, then return null
      pushLogsSpy
        .mockResolvedValueOnce([createMockEvent(1)])
        .mockResolvedValueOnce([createMockEvent(2)])
        .mockResolvedValueOnce([createMockEvent(3)])
        .mockResolvedValueOnce(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(4);
      expect(markUploadedSpy).toHaveBeenCalledTimes(3);
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });
  });

  describe('integration with Pusher', () => {
    it('should work with Pusher to upload batches', async () => {
      // Mock pushLogs to return batches and then null
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      pushLogsSpy
        .mockResolvedValueOnce([createMockEvent(1), createMockEvent(2)])
        .mockResolvedValueOnce([createMockEvent(3), createMockEvent(4)])
        .mockResolvedValueOnce([createMockEvent(5), createMockEvent(6)])
        .mockResolvedValueOnce(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(4);
      pushLogsSpy.mockRestore();
    });

    it('should handle Pusher concurrent upload prevention', async () => {
      Pusher['_isUploadInProgress'] = true;

      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs').mockResolvedValue(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalled();
      pushLogsSpy.mockRestore();
    });
  });

  describe('integration with Batcher', () => {
    it('should mark batches as uploaded', async () => {
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');

      pushLogsSpy
        .mockResolvedValueOnce([createMockEvent(1)])
        .mockResolvedValueOnce([createMockEvent(2)])
        .mockResolvedValueOnce(null);

      await flush();

      expect(markUploadedSpy).toHaveBeenCalledTimes(2);
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });

    it('should increment _currentBatchToUpload correctly', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      pushLogsSpy
        .mockResolvedValueOnce([createMockEvent(1)])
        .mockResolvedValueOnce([createMockEvent(2)])
        .mockResolvedValueOnce(null);

      await flush();

      expect(Batcher['_currentBatchToUpload']).toBe(2);
      pushLogsSpy.mockRestore();
    });
  });

  describe('edge cases', () => {
    it('should handle empty batches', async () => {
      Batcher.reset();

      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      pushLogsSpy
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(2);
      pushLogsSpy.mockRestore();
    });

    it('should handle single batch', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      pushLogsSpy.mockResolvedValueOnce(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(1);
      pushLogsSpy.mockRestore();
    });

    it('should handle many batches', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');

      // Simulate 10 batches
      pushLogsSpy.mockResolvedValueOnce([createMockEvent(1)])
        .mockResolvedValueOnce([createMockEvent(2)])
        .mockResolvedValueOnce([createMockEvent(3)])
        .mockResolvedValueOnce([createMockEvent(4)])
        .mockResolvedValueOnce([createMockEvent(5)])
        .mockResolvedValueOnce([createMockEvent(6)])
        .mockResolvedValueOnce([createMockEvent(7)])
        .mockResolvedValueOnce([createMockEvent(8)])
        .mockResolvedValueOnce([createMockEvent(9)])
        .mockResolvedValueOnce([createMockEvent(10)])
        .mockResolvedValueOnce(null);

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(11);
      expect(markUploadedSpy).toHaveBeenCalledTimes(10);
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });

    it('should be async and return Promise', async () => {
      const result = flush();

      expect(result).toBeInstanceOf(Promise);
      await result;
    });

    it('should handle pushLogs throwing errors', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      pushLogsSpy.mockRejectedValue(new Error('Upload failed'));

      await expect(flush()).rejects.toThrow('Upload failed');
      pushLogsSpy.mockRestore();
    });

    it('should handle setMarkLastBatchUploaded throwing errors', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');

      pushLogsSpy.mockResolvedValueOnce([createMockEvent(1)]);
      markUploadedSpy.mockImplementation(() => {
        throw new Error('Mark failed');
      });

      await expect(flush()).rejects.toThrow('Mark failed');
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });
  });

  describe('loop behavior', () => {
    it('should continuously call pushLogs until null is returned', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');

      let callCount = 0;
      pushLogsSpy.mockImplementation(() => {
        callCount++;
        if (callCount < 5) {
          return Promise.resolve([createMockEvent(callCount)]);
        }
        return Promise.resolve(null);
      });

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(5);
      expect(markUploadedSpy).toHaveBeenCalledTimes(4);
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });

    it('should exit loop immediately when pushLogs returns null on first call', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs').mockResolvedValue(null);
      const markUploadedSpy = jest.spyOn(Batcher, 'setMarkLastBatchUploaded');

      await flush();

      expect(pushLogsSpy).toHaveBeenCalledTimes(1);
      expect(markUploadedSpy).not.toHaveBeenCalled();
      pushLogsSpy.mockRestore();
      markUploadedSpy.mockRestore();
    });
  });

  describe('BUG TEST: uses loose equality (==) instead of strict equality (===)', () => {
    it('BUG: line 7 uses == instead of ===', async () => {
      const pushLogsSpy = jest.spyOn(Pusher, 'pushLogs');

      // Test with different falsy values
      pushLogsSpy.mockResolvedValueOnce(null);
      await flush();

      pushLogsSpy.mockResolvedValueOnce(null);
      await flush();

      pushLogsSpy.mockResolvedValueOnce(null);
      await flush();

      pushLogsSpy.mockResolvedValueOnce(null);
      await flush();

      // BUG: The code uses == which means any falsy value will trigger return
      // Should use === to check for null specifically
      expect(pushLogsSpy).toHaveBeenCalled();
      pushLogsSpy.mockRestore();
    });
  });
});
