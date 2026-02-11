import { Batcher } from "./Batcher";
import { flush } from "./Flush";
import type { Event, BatchPayload, ClientEvent } from "./types";
import { getSessionData } from "./Session";
import { AnonymousId } from "./AnonymousId";
import { Configuration } from "./Configuration";

export class Pusher {
  private static _isUploadInProgress = false;

  private static get endpoint(): string {
    return Configuration.endpoint;
  }

  /**
   * Transform internal Event array to BatchPayload format
   */
  private static transformBatch(batch: Event[]): BatchPayload {
    const sessionData = getSessionData();

    const clientEvents: ClientEvent[] = batch.map((event) => ({
      event_id: event.eventId,
      event_name: event.eventName,
      event_timestamp: event.at,
      user_id: event.user,
      anon_id: event.anon_id,
      properties: JSON.stringify(event.properties),
    }));

    return {
      session: sessionData,
      events: clientEvents,
    };
  }

  static async pushLogs(useBeacon: boolean = false): Promise<Event[] | null> {
    if (this._isUploadInProgress) return null;
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

    if (useBeacon) {
      // Use navigator.sendBeacon() for reliable delivery during page unload
      // Beacon is fire-and-forget - we don't wait for response
      this.sendWithBeacon(payload);
      this._isUploadInProgress = false;
      return null; // Don't mark batch as uploaded since we don't know if it succeeded
    } else {
      // Use normal fetch for regular operations
      const success = await this.sendNormally(payload);
      this._isUploadInProgress = false;
      return success ? batch : null;
    }
  }

  private static sendWithBeacon(payload: BatchPayload): boolean {
    if (typeof navigator !== "undefined" && navigator.sendBeacon) {
      try {
        const blob = new Blob([JSON.stringify(payload)], {
          type: "application/json",
        });
        return navigator.sendBeacon(this.endpoint, blob);
      } catch (error) {
        console.error("Beacon send failed:", error);
        return false;
      }
    }
    return false;
  }

  private static async sendNormally(payload: BatchPayload): Promise<boolean> {
    try {
      const response = await fetch(this.endpoint, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(payload),
      });
      return response.ok;
    } catch (error) {
      console.error("Fetch failed:", error);
      return false;
    }
  }

  static async startScheduler(time: number) {
    setInterval(() => {
      flush();
    }, time);
  }
}