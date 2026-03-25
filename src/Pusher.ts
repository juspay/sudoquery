import { Batcher } from "./Batcher";
import { flush } from "./Flush";
import type { Event, BatchPayload, ClientEvent } from "./types";
import { getSessionData } from "./Session";
import { Configuration } from "./Configuration";
import { getNetwork, getPlatform, isPlatformInitialized } from "./platform";

export class Pusher {
  private static _isUploadInProgress = false;

  private static get endpoint(): string {
    return Configuration.endpoint;
  }

  /**
   * Get headers for API requests.
   */
  private static getHeaders(): Record<string, string> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };

    if (Configuration.token) {
      headers["Authorization"] = `Bearer ${Configuration.token}`;
    }

    return headers;
  }

  /**
   * Transform internal Event array to BatchPayload format
   */
  private static async transformBatch(batch: Event[]): Promise<BatchPayload> {
    const sessionData = await getSessionData();

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
    const payload = await this.transformBatch(batch);
    if (!payload) {
      this._isUploadInProgress = false;
      return null;
    }

    // Use platform network adapter
    if (useBeacon && isPlatformInitialized()) {
      const network = getNetwork();
      const headers = this.getHeaders();

      // For unreliable delivery (page unload / app background)
      network.sendUnreliable(this.endpoint, payload, headers);
      this._isUploadInProgress = false;
      return null; // Don't mark batch as uploaded since we don't know if it succeeded
    } else if (isPlatformInitialized()) {
      const network = getNetwork();
      const headers = this.getHeaders();
      const success = await network.send(this.endpoint, payload, headers);
      this._isUploadInProgress = false;
      return success ? batch : null;
    } else {
      // Fallback for when platform isn't initialized (shouldn't happen)
      this._isUploadInProgress = false;
      return null;
    }
  }

  static async startScheduler(time: number) {
    setInterval(() => {
      flush();
    }, time);
  }
}