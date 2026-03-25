import { Pusher } from "../src/Pusher";
import { Batcher } from "../src/Batcher";
import { Configuration } from "../src/Configuration";
import { initializePlatform, resetPlatform } from "../src/platform";
import type { Event } from "../src/types";

// Mock Flush module
jest.mock("../src/Flush", () => ({
  flush: jest.fn().mockResolvedValue(undefined),
}));

// Mock Session module
jest.mock("../src/Session", () => ({
  getSessionData: jest.fn(() => Promise.resolve({
    session_id: "mock-session-id",
    distinct_id_snapshot: "test-client-id",
    device_type: "desktop",
    platform: "Unknown",
    browser: "Unknown",
    country: "",
    city: "",
    ip_address: null,
    user_agent: "Unknown",
  })),
}));

// Mock global fetch for sendNormally
global.fetch = jest.fn(() =>
  Promise.resolve({
    ok: true,
    json: () => Promise.resolve({}),
  })
) as jest.Mock;

// Mock navigator object for sendWithBeacon
(global as any).navigator = {
  sendBeacon: jest.fn(() => true),
};

describe("Pusher", () => {
  const createMockEvent = (id: number): Event => ({
    eventName: `event_${id}`,
    eventId: crypto.randomUUID(),
    properties: { id },
    user: `user_${id}`,
    anon_id: crypto.randomUUID(),
    at: Date.now(),
  });

  beforeEach(async () => {
    // Reset state before each test
    Configuration.setBatchSize(10);
    jest.clearAllMocks();
    Batcher.reset();
    resetPlatform();
    await initializePlatform();
  });

  describe("pushLogs", () => {
    it("should return null when no batches are available", async () => {
      const result = await Pusher.pushLogs();
      expect(result).toBeNull();
    });

    it("should fetch batch from Batcher", async () => {
      // Add some events
      for (let i = 1; i <= 5; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const result = await Pusher.pushLogs();
      expect(result).not.toBeNull();
      expect(result).toHaveLength(5);
    });

    it("should call fetch endpoint with batch data", async () => {
      // Add events to create a batch
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 3; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      await Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        Configuration.endpoint,
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({
            "Content-Type": "application/json",
          }),
        })
      );
    });

    it("should handle successful upload", async () => {
      // Add events to create a batch
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 3; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const result = await Pusher.pushLogs();
      expect(result).not.toBeNull();
      expect(result).toHaveLength(3);
    });

    it("should handle failed upload", async () => {
      // Mock fetch failure
      (global.fetch as jest.Mock).mockRejectedValueOnce(new Error("Network error"));

      // Add events to create a batch
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 3; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const result = await Pusher.pushLogs();
      expect(result).toBeNull();
    });

    it("should handle multiple sequential calls", async () => {
      Configuration.setBatchSize(2);

      // Add 4 events (2 batches)
      for (let i = 1; i <= 4; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      // First push
      const result1 = await Pusher.pushLogs();
      expect(result1).toHaveLength(2);

      // Mark first batch as uploaded
      Batcher.setMarkLastBatchUploaded();

      // Second push
      const result2 = await Pusher.pushLogs();
      expect(result2).toHaveLength(2);
    });

    it("should handle useBeacon parameter", async () => {
      Configuration.setBatchSize(2);

      for (let i = 1; i <= 2; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const result = await Pusher.pushLogs(true);

      // Beacon/unreliable delivery returns null
      expect(result).toBeNull();
      // In Node.js, we use fetch with fire-and-forget instead of sendBeacon
      // The fetch should have been called
    });

    it("should be async and return Promise", async () => {
      const result = Pusher.pushLogs();

      expect(result).toBeInstanceOf(Promise);
      await result;
    });
  });

  describe("startScheduler", () => {
    it("should start a scheduler with the given interval", () => {
      const { flush } = require("../src/Flush");
      jest.useFakeTimers();

      Pusher.startScheduler(1000);

      // Advance time by 1 second
      jest.advanceTimersByTime(1000);

      expect(flush).toHaveBeenCalled();

      jest.useRealTimers();
    });

    it("should call flush repeatedly at the specified interval", () => {
      const { flush } = require("../src/Flush");
      jest.useFakeTimers();

      Pusher.startScheduler(500);

      // Advance time by 1.5 seconds (3 intervals)
      jest.advanceTimersByTime(1500);

      expect(flush).toHaveBeenCalledTimes(3);

      jest.useRealTimers();
    });
  });

  describe("static behavior", () => {
    it("should not require instantiation", () => {
      expect(() => {
        Pusher.pushLogs();
        Pusher.startScheduler(1000);
      }).not.toThrow();
    });

    it("should maintain state across calls", async () => {
      Configuration.setBatchSize(2);

      Batcher.addToBatch(createMockEvent(1));
      Batcher.addToBatch(createMockEvent(2));

      const result = await Pusher.pushLogs();

      expect(result).not.toBeNull();
    });
  });

  describe("integration with Batcher", () => {
    it("should work with Batcher to fetch batches", async () => {
      Configuration.setBatchSize(3);

      // Add events
      for (let i = 1; i <= 6; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      // First push
      const result1 = await Pusher.pushLogs();
      expect(result1).toHaveLength(3);

      // Mark first batch as uploaded
      Batcher.setMarkLastBatchUploaded();

      // Second push
      const result2 = await Pusher.pushLogs();
      expect(result2).toHaveLength(3);
    });
  });

  describe("setEndpoint", () => {
    it("should allow changing the endpoint", async () => {
      const newEndpoint = "https://api.example.com/events";
      Configuration.setEndpoint(newEndpoint);

      // Verify by checking if fetch is called with the new endpoint
      Configuration.setBatchSize(2);
      for (let i = 1; i <= 2; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      await Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        newEndpoint,
        expect.any(Object)
      );
    });
  });

  describe("edge cases", () => {
    it("should handle empty batches", async () => {
      // Empty batches return null from fetchBatchToUpload
      const result = await Pusher.pushLogs();
      expect(result).toBeNull();
    });
  });
});