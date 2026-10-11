import { Batcher } from "./Batcher";
import { flush } from "./Flush";
import type { Event, BatchPayload } from "./types";
import { getSystemProperties } from "./Session";
import { Configuration } from "./Configuration";

export class Pusher {
  private static _isUploadInProgress = false;

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
      // Use keepalive fetch during unload so collector-required headers are sent.
      this.sendWithKeepalive(payload);
      this._isUploadInProgress = false;
      return null; // Don't mark batch as uploaded since we don't know if it succeeded
    } else {
      // Use normal fetch for regular operations
      const success = await this.sendNormally(payload);
      this._isUploadInProgress = false;
      return success ? batch : null;
    }
  }

  private static sendWithKeepalive(payload: BatchPayload): boolean {
    if (typeof fetch === "undefined") {
      return false;
    }

    const headers = this.buildHeaders(payload);
    if (!headers) {
      return false;
    }

    try {
      void fetch(this.endpoint, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
        keepalive: true,
      });
      return true;
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
    const orgId = Configuration.orgId ?? payload.events[0]?.org_id ?? null;

    if (!orgId) {
      console.error("Cannot send analytics batch: orgId is required by the collector.");
      return null;
    }

    const projectId = Configuration.projectId ?? payload.events[0]?.project_id ?? null;

    if (!projectId) {
      console.error("Cannot send analytics batch: projectId is required by the collector.");
      return null;
    }

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      ...Configuration.headers,
      "x-org-id": orgId,
      "x-project-id": projectId,
    };

    if (Configuration.token) {
      headers.Authorization  = `Bearer ${Configuration.token}`;
    }

    return headers;
  }

  static async startScheduler(time: number) {
    setInterval(() => {
      flush();
    }, time);
  }
}
