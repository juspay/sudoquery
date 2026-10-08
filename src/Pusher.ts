import { Batcher } from "./Batcher";
import { flush } from "./Flush";
import type { Event, BatchPayload } from "./types";
import { getSystemProperties } from "./Session";
import { Configuration } from "./Configuration";

/**
 * Observer for upload outcomes. Lets optional features react to delivery
 * without Pusher depending on them.
 */
export interface DeliveryListener {
  /** A batch reached the collector. */
  onDelivered?(batch: Event[]): void;
  /** A batch could not be delivered and stays queued for retry. */
  onFailed?(batch: Event[]): void;
  /** The page is being hidden and these batches are being sent with keepalive. */
  onUnloadSend?(batches: Event[][]): void;
}

export class Pusher {
  private static _isUploadInProgress = false;
  private static _failedAttempts = 0;
  private static _retryAt = 0;
  private static _retryTimer: ReturnType<typeof setTimeout> | null = null;
  private static _listeners: DeliveryListener[] = [];

  static addDeliveryListener(listener: DeliveryListener): void {
    this._listeners.push(listener);
  }

  private static notify(call: (listener: DeliveryListener) => void): void {
    for (const listener of this._listeners) {
      try {
        call(listener);
      } catch (error) {
        console.error("Delivery listener error:", error);
      }
    }
  }

  private static get endpoint(): string {
    return Configuration.endpoint;
  }

  /**
   * Wrap queued collector events in the collector batch format.
   */
  private static transformBatch(batch: Event[]): BatchPayload {
    return {
      events: batch,
      system_properties: getSystemProperties(),
    };
  }

  static async pushLogs(useBeacon: boolean = false): Promise<Event[] | null> {
    if (useBeacon) {
      // Page is being hidden: send everything queued, not just the next batch.
      this.sendAllWithKeepalive();
      return null;
    }

    // While backing off after a failure, leave the queue for the retry timer.
    if (this._isUploadInProgress || this.isBackingOff()) return null;
    this._isUploadInProgress = true;
    const batch = Batcher.fetchBatchToUpload();
    if (!batch) {
      this._isUploadInProgress = false;
      return null;
    }

    // Transform batch to new format
    const payload = this.transformBatch(batch);
    if (!payload) {
      this._isUploadInProgress = false;
      return null;
    }

    const success = await this.sendNormally(payload);
    this._isUploadInProgress = false;
    if (!success) {
      this.notify((listener) => listener.onFailed?.(batch));
      this.scheduleRetry();
      return null;
    }
    this._failedAttempts = 0;
    this.notify((listener) => listener.onDelivered?.(batch));
    return batch;
  }

  private static isBackingOff(): boolean {
    return Date.now() < this._retryAt;
  }

  /**
   * Back off exponentially (retryBaseDelay, doubling, capped at retryMaxDelay) with jitter so many
   * clients recovering from the same outage don't retry in lockstep, then retry.
   */
  private static scheduleRetry(): void {
    this._failedAttempts++;
    const maxDelay = Math.min(
      Configuration.retryMaxDelay,
      Configuration.retryBaseDelay * 2 ** (this._failedAttempts - 1),
    );
    const delay = maxDelay / 2 + Math.random() * (maxDelay / 2);
    this._retryAt = Date.now() + delay;

    if (this._retryTimer !== null) clearTimeout(this._retryTimer);
    this._retryTimer = setTimeout(() => {
      this._retryTimer = null;
      this._retryAt = 0;
      flush(false).catch((err) => console.error("Retry flush error:", err));
    }, delay);
  }

  /**
   * Send all pending batches with keepalive so they survive page unload.
   * Batches leave the queue before sending so a later flush can't resend them;
   * if the page is still alive when a send fails, the batch is requeued.
   */
  private static sendAllWithKeepalive(): void {
    const batches = Batcher.takeAllPending(this._isUploadInProgress);
    if (batches.length > 0) this.notify((listener) => listener.onUnloadSend?.(batches));
    for (const batch of batches) {
      void this.sendWithKeepalive(this.transformBatch(batch)).then((success) => {
        if (success) {
          this.notify((listener) => listener.onDelivered?.(batch));
        } else {
          this.notify((listener) => listener.onFailed?.(batch));
          Batcher.requeue(batch, this._isUploadInProgress);
          // Several batches can fail together; count that as one failed attempt.
          if (!this.isBackingOff()) this.scheduleRetry();
        }
      });
    }
  }

  private static async sendWithKeepalive(payload: BatchPayload): Promise<boolean> {
    if (typeof fetch === "undefined") {
      return false;
    }

    const headers = this.buildHeaders(payload);
    if (!headers) {
      return false;
    }

    try {
      const response = await fetch(this.endpoint, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
        keepalive: true,
      });
      return response.ok;
    } catch (error) {
      console.error("Keepalive fetch failed:", error);
      return false;
    }
  }

  private static async sendNormally(payload: BatchPayload): Promise<boolean> {
    const headers = this.buildHeaders(payload);
    if (!headers) {
      return false;
    }

    try {
      const response = await fetch(this.endpoint, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
      });
      return response.ok;
    } catch (error) {
      console.error("Fetch failed:", error);
      return false;
    }
  }

  private static buildHeaders(payload: BatchPayload): Record<string, string> | null {
    const tenantId = Configuration.tenantId ?? payload.events[0]?.tenant_id ?? null;

    if (!tenantId) {
      console.error("Cannot send analytics batch: tenantId is required by the collector.");
      return null;
    }

    const workspaceId = Configuration.workspaceId ?? payload.events[0]?.workspace_id ?? null;
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      ...Configuration.headers,
      "x-tenant-id": tenantId,
    };

    if (workspaceId) {
      headers["x-workspace-id"] = workspaceId;
    }

    if (Configuration.token) {
      headers.Authorization  = `Bearer ${Configuration.token}`;
    }

    return headers;
  }

  /**
   * Reset upload and backoff state. Useful for testing.
   */
  static reset(): void {
    if (this._retryTimer !== null) clearTimeout(this._retryTimer);
    this._retryTimer = null;
    this._retryAt = 0;
    this._failedAttempts = 0;
    this._isUploadInProgress = false;
    this._listeners = [];
  }

  static async startScheduler(time: number) {
    setInterval(() => {
      flush();
    }, time);
  }
}
