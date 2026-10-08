import { Pusher } from "../src/Pusher";
import { Batcher } from "../src/Batcher";
import { Configuration } from "../src/Configuration";
import { createMockEvent } from "./testUtils";

// Mock Flush module
jest.mock("../src/Flush", () => ({
  flush: jest.fn().mockResolvedValue(undefined),
}));

// Mock Session module
jest.mock("../src/Session", () => ({
  getSystemProperties: jest.fn(() => ({
    geo: null,
    timezone: "Asia/Kolkata",
  })),
}));

// Mock global fetch for sendNormally
global.fetch = jest.fn(() =>
  Promise.resolve({
    ok: true,
    json: () => Promise.resolve({}),
  })
) as jest.Mock;

describe("Pusher", () => {
  beforeEach(() => {
    // Reset state before each test
    Configuration.reset();
    Configuration.setTenantId("tenant-1");
    jest.clearAllMocks();
    Batcher.reset();
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
      Configuration.setEndpoint("http://localhost:3000/batch");
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 3; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      await Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        "http://localhost:3000/batch",
        expect.objectContaining({
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "x-tenant-id": "tenant-1",
          },
        })
      );

      const [, request] = (fetch as jest.Mock).mock.calls[0];
      const payload = JSON.parse(request.body);
      expect(payload).toEqual({
        events: expect.arrayContaining([
          expect.objectContaining({
            envelop_version: "1.0",
            name: "event_1",
            org_id: "tenant-1",
            actor_id: "user_1",
            properties: { id: 1 },
          }),
        ]),
        system_properties: {
          geo: null,
          timezone: "Asia/Kolkata",
        },
      });
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
      const consoleErrorSpy = jest.spyOn(console, "error").mockImplementation(() => undefined);
      (global.fetch as jest.Mock).mockRejectedValueOnce(new Error("Network error"));

      // Add events to create a batch
      Configuration.setBatchSize(3);
      for (let i = 1; i <= 3; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      const result = await Pusher.pushLogs();
      expect(result).toBeNull();
      consoleErrorSpy.mockRestore();
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

      // Keepalive requests are fire-and-forget during unload.
      expect(result).toBeNull();
      expect(fetch).toHaveBeenCalledWith(
        "http://localhost:3000/batch",
        expect.objectContaining({
          keepalive: true,
          headers: expect.objectContaining({
            "x-tenant-id": "tenant-1",
          }),
        })
      );
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
      jest.useFakeTimers();
      expect(() => {
        Pusher.pushLogs();
        Pusher.startScheduler(1000);
      }).not.toThrow();
      jest.useRealTimers();
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
    it("should allow changing the endpoint", () => {
      const newEndpoint = "https://api.example.com/events";
      Configuration.setEndpoint(newEndpoint);

      // Verify by checking if fetch is called with the new endpoint
      Configuration.setBatchSize(2);
      for (let i = 1; i <= 2; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        newEndpoint,
        expect.any(Object)
      );
    });
  });

  describe("custom headers", () => {
    it("should include custom headers in fetch request", async () => {
      Configuration.setEndpoint("http://localhost:3000/batch");
      Configuration.setBatchSize(2);
      Configuration.setHeaders({
        "X-Api-Key": "test-api-key",
        "X-Request-Id": "12345",
      });

      for (let i = 1; i <= 2; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      await Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        "http://localhost:3000/batch",
        expect.objectContaining({
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Api-Key": "test-api-key",
            "X-Request-Id": "12345",
            "x-tenant-id": "tenant-1",
          },
        })
      );
    });

    it("should include Authorization header when token is set", async () => {
      Configuration.setEndpoint("http://localhost:3000/batch");
      Configuration.setBatchSize(2);
      Configuration.setHeaders({
        "X-Custom-Header": "custom-value",
      });
      Configuration.setToken("my-auth-token");

      for (let i = 1; i <= 2; i++) {
        Batcher.addToBatch(createMockEvent(i));
      }

      await Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        "http://localhost:3000/batch",
        expect.objectContaining({
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Custom-Header": "custom-value",
            "x-tenant-id": "tenant-1",
            "Authorization": "Bearer my-auth-token",
          },
        })
      );
    });

    it("should include workspace header when workspaceId is set", async () => {
      Configuration.setBatchSize(2);
      Configuration.setWorkspaceId("workspace-1");

      for (let i = 1; i <= 2; i++) {
        Batcher.addToBatch(createMockEvent(i, { project_id: "workspace-1" }));
      }

      await Pusher.pushLogs();

      expect(fetch).toHaveBeenCalledWith(
        "http://localhost:3000/batch",
        expect.objectContaining({
          headers: expect.objectContaining({
            "x-tenant-id": "tenant-1",
            "x-workspace-id": "workspace-1",
          }),
        })
      );
    });
  });

  describe("edge cases", () => {
    it("should handle empty batches", async () => {
      // Empty batches return null from fetchBatchToUpload
      const result = await Pusher.pushLogs();
      expect(result).toBeNull();
    });

    it("should not upload without tenantId", async () => {
      const consoleErrorSpy = jest.spyOn(console, "error").mockImplementation(() => undefined);
      Configuration.setTenantId(null);
      Batcher.addToBatch(createMockEvent(1, { org_id: "" }));

      const result = await Pusher.pushLogs();

      expect(result).toBeNull();
      expect(fetch).not.toHaveBeenCalled();
      consoleErrorSpy.mockRestore();
    });
  });
});
