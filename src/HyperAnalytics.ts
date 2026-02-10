import { JSONSerializable, Event } from "./types";
import { containsNonPrimitives } from "./TypeValidator";
import { Batcher } from "./Batcher";
import { SuperProperties } from "./SuperProperties";
import { flush } from "./Flush";
import { AnonymousId } from "./AnonymousId";
import { Pusher } from "./Pusher";

class HyperAnalytics {
    private static didInit = false;
    private static currentUser: string | null = null;
    private static currentGroup: string | null = null;

    static init() {
      if(this.didInit) return;
      this.didInit = true;

      // Flush events when page is unloaded (user navigates away or closes tab)
      // Use pagehide event which is more reliable than visibilitychange for page unload
      // Beacon/keepalive ensures events are delivered even during page unload
      if (typeof document!== 'undefined') {
        document.addEventListener("visibilitychange", () => {
          // 'hidden' means the user switched tabs, minimized, or closed the browser.
          // This is your last reliable chance to send data.
          if (document.visibilityState === "hidden") {
            flush(true).catch(err => console.error('Flush on pagehide error:', err));
          }
        });
      }
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
