import { JSONSerializable, Event } from "./types";
import { Batcher } from "./Batcher";
import { SuperProperties } from "./SuperProperties";
import { flush as flushEvents } from "./Flush";
import { AnonymousId } from "./AnonymousId";
import { Pusher } from "./Pusher";
import { Configuration } from "./Configuration";
import { getSessionId } from "./Session";
import { generateUuid } from "./Uuid";

export interface SudoQueryConfig {
  flushInterval?: number;
  batchSize?: number;
  endpoint?: string;
  token?: string;
  headers?: Record<string, string>;
  orgId?: string | null;
  projectId?: string | null;
  source?: string | null;
  sessionId?: string | null;
}

class SudoQuery {
  private static didInit = false;
  private static currentUser: string | null = null;
  private static flushTimer: ReturnType<typeof setInterval> | null = null;

  static init(config?: SudoQueryConfig) {
    if (this.didInit) return;
    this.didInit = true;

    // Configure batch size if provided
    if (config?.batchSize !== undefined) {
      Configuration.setBatchSize(config.batchSize);
    }

    // Configure endpoint if provided
    if (config?.endpoint !== undefined) {
      Configuration.setEndpoint(config.endpoint);
    }

    // Configure token if provided
    if (config?.token !== undefined) {
      Configuration.setToken(config.token);
    }

    // Configure custom headers if provided
    if (config?.headers !== undefined) {
      Configuration.setHeaders(config.headers);
    }

    if (config?.orgId !== undefined) {
      warnIfNotSlug("orgId", config.orgId);
      Configuration.setOrgId(config.orgId);
    }

    if (config?.projectId !== undefined) {
      warnIfNotSlug("projectId", config.projectId);
      Configuration.setProjectId(config.projectId);
    }

    if (config?.source !== undefined) {
      Configuration.setSource(config.source);
    }

    if (config?.sessionId !== undefined) {
      Configuration.setSessionId(config.sessionId);
    }

    // Start periodic auto-flush if configured
    if (config?.flushInterval !== undefined && config.flushInterval > 0) {
      Configuration.setFlushInterval(config.flushInterval);
      this.startPeriodicFlush(config.flushInterval);
    }

    // Flush events when page is unloaded (user navigates away or closes tab).
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") {
          this.flush(true).catch((err) => console.error("Flush on pagehide error:", err));
        }
      });
    }
  }

  private static startPeriodicFlush(intervalMs: number): void {
    if (this.flushTimer !== null) return;
    this.flushTimer = setInterval(() => {
      this.flush(false).catch((err) => console.error("Periodic flush error:", err));
    }, intervalMs);
  }

  /**
   * Check if the SDK has been initialized
   */
  static get isInitialized(): boolean {
    return this.didInit;
  }

  /**
   * Get the current batch size
   */
  static get batchSize(): number {
    return Configuration.batchSize;
  }

  /**
   * Get the current endpoint URL
   */
  static get endpoint(): string {
    return Configuration.endpoint;
  }

  /**
   * Add a property to super properties (included in all events)
   */
  static setSuperProperty(key: string, value: JSONSerializable): void {
    SuperProperties.addToSuperProperties(key, value);
  }

  /**
   * Get all current super properties
   */
  static getSuperProperties(): Record<string, JSONSerializable> {
    return SuperProperties.getSuperProperties();
  }

  /**
   * Clear all super properties
   */
  static clearSuperProperties(): void {
    SuperProperties.clearSuperProperties();
  }

  /**
   * Flush all pending events to the server
   * @param useBeacon - Use fetch keepalive for more reliable delivery during page unload
   */
  static async flush(useBeacon: boolean = false): Promise<void> {
    await flushEvents(useBeacon);
  }

  /**
   * Set the user ID for all subsequent events
   * @param userId - The user identifier
   */
  static setUser(userId: string): void {
    this.currentUser = userId;
  }

  /**
   * Remove the current user ID (resets to null)
   */
  static removeUser(): void {
    this.currentUser = null;
  }

  /**
   * Get the current user ID
   * @returns The current user ID or null if not set
   */
  static getUser(): string | null {
    return this.currentUser;
  }

  static track(eventName: string, properties?: JSONSerializable): void {
    const props = properties ?? {};
    const orgId = Configuration.orgId;
    if (!orgId || orgId.trim().length === 0) {
      throw new Error("orgId is required before tracking events");
    }
    const projectId = Configuration.projectId;
    if (!projectId || projectId.trim().length === 0) {
      throw new Error("projectId is required before tracking events");
    }

    const superProperties = SuperProperties.getSuperProperties();
    const mergedProperties = mergeProperties(props, superProperties);

    const event: Event = {
      envelop_version: "1.0",
      id: generateUuid(),
      name: eventName.toString(),
      org_id: orgId,
      project_id: projectId,
      session_id: Configuration.sessionId ?? getSessionId(),
      anon_id: AnonymousId.getOrCreate(),
      actor_id: this.currentUser,
      source: Configuration.source,
      occured_at: new Date().toISOString(),
      properties: mergedProperties,
      correlation_id: null,
      trace_id: null,
      system_properties: null,
    };

    Batcher.addToBatch(event);
  }
}

function mergeProperties(
  properties: JSONSerializable,
  defaults: Record<string, JSONSerializable>,
): JSONSerializable {
  if (isJsonRecord(properties)) {
    return {
      ...defaults,
      ...properties,
    };
  }

  return properties;
}

const SLUG_PATTERN = /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/;

/**
 * Org and project ids are server-generated slugs (e.g. "acme-store-k3x9qa").
 * Warn — never throw — when a provided id deviates from that shape so the
 * misconfiguration is visible without breaking the host application.
 */
function warnIfNotSlug(field: "orgId" | "projectId", value: string | null): void {
  if (value === null || SLUG_PATTERN.test(value)) {
    return;
  }

  console.warn(
    `SudoQuery: config option "${field}" value "${value}" does not match the expected ` +
      `server-generated slug shape /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/ ` +
      `(e.g. "acme-store-k3x9qa"); the collector may reject events carrying it.`,
  );
}

function isJsonRecord(value: JSONSerializable): value is Record<string, JSONSerializable> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export { SudoQuery };
