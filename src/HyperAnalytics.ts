import { JSONSerializable, Event } from "./types";
import { containsNonPrimitives } from "./TypeValidator";
import { Batcher } from "./Batcher";
import { SuperProperties } from "./SuperProperties";
import { flush as flushEvents } from "./Flush";
import { AnonymousId } from "./AnonymousId";
import { Pusher } from "./Pusher";
import { Configuration } from "./Configuration";

class HyperAnalytics {
    private static didInit = false;
    private static currentUser: string | null = null;
    private static currentGroup: string | null = null;
    private static flushTimer: ReturnType<typeof setInterval> | null = null;

    static init(config?: { flushInterval?: number; batchSize?: number; endpoint?: string }) {
      if(this.didInit) return;
      this.didInit = true;

      // Configure batch size if provided
      if (config?.batchSize !== undefined) {
        Configuration.setBatchSize(config.batchSize);
      }

      // Configure endpoint if provided
      if (config?.endpoint !== undefined) {
        Configuration.setEndpoint(config.endpoint);
      }

      // Start periodic auto-flush if configured
      if (config?.flushInterval !== undefined && config.flushInterval > 0) {
        Configuration.setFlushInterval(config.flushInterval);
        this.startPeriodicFlush(config.flushInterval);
      }

      // Flush events when page is unloaded (user navigates away or closes tab)
      // Use pagehide event which is more reliable than visibilitychange for page unload
      // Beacon/keepalive ensures events are delivered even during page unload
      if (typeof document!== 'undefined') {
        document.addEventListener("visibilitychange", () => {
          // 'hidden' means the user switched tabs, minimized, or closed the browser.
          // This is your last reliable chance to send data.
          if (document.visibilityState === "hidden") {
            this.stopPeriodicFlush();
            this.flush(true).catch(err => console.error('Flush on pagehide error:', err));
          }
        });
      }
    }

    private static startPeriodicFlush(intervalMs: number): void {
      if (this.flushTimer !== null) return;
      this.flushTimer = setInterval(() => {
        this.flush(false).catch(err => console.error('Periodic flush error:', err));
      }, intervalMs);
    }

    private static stopPeriodicFlush(): void {
      if (this.flushTimer !== null) {
        clearInterval(this.flushTimer);
        this.flushTimer = null;
      }
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
     * @param useBeacon - Use navigator.sendBeacon for more reliable delivery during page unload
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

    /**
     * Set the group ID for all subsequent events
     * @param groupId - The group identifier
     */
    static setGroup(groupId: string): void {
      this.currentGroup = groupId;
    }

    /**
     * Remove the current group ID (resets to null)
     */
    static removeGroup(): void {
      this.currentGroup = null;
    }

    /**
     * Get the current group ID
     * @returns The current group ID or null if not set
     */
    static getGroup(): string | null {
      return this.currentGroup;
    }

    static track(eventName: String, properties: JSONSerializable){
      if(containsNonPrimitives(properties)) throw new Error("only primitives are allowed as properties");

      // Create session details (empty for now)
      const sessionDetails: Record<string, JSONSerializable> = {};

      // Get super properties
      const superProperties = SuperProperties.getSuperProperties();

      // Merge all properties (user properties take precedence over super properties)
      const mergedProperties: Record<string, JSONSerializable> = {
        ...superProperties,
        ...(typeof properties === 'object' && properties !== null && !Array.isArray(properties) ? properties : {}),
        ...sessionDetails,
      };

      // Create event object with current user and group
      const event: Event = {
        eventName: eventName.toString(),
        properties: mergedProperties,
        user: this.currentUser,
        group: this.currentGroup,
        anon_id: AnonymousId.getOrCreate(),
        eventId: crypto.randomUUID(),
        at: Date.now(),
      };

      // Add to batch
      Batcher.addToBatch(event);
    }
}

export { HyperAnalytics };
