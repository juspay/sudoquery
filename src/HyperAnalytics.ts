import { JSONSerializable, Event } from "./types";
import { containsNonPrimitives } from "./TypeValidator";
import { Batcher } from "./Batcher";
import { SuperProperties } from "./SuperProperties";
import { flush as flushEvents } from "./Flush";
import { AnonymousId } from "./AnonymousId";
import { Pusher } from "./Pusher";
import { Configuration } from "./Configuration";
import {
  initializePlatform,
  getLifecycle,
  getPlatform,
  isPlatformInitialized,
  resetPlatform,
} from "./platform";

export interface HyperAnalyticsConfig {
  flushInterval?: number;
  batchSize?: number;
  endpoint?: string;
  token?: string;
}

class HyperAnalytics {
    private static didInit = false;
    private static currentUser: string | null = null;
    private static flushTimer: ReturnType<typeof setInterval> | null = null;
    private static cleanupFns: (() => void)[] = [];

    /**
     * Initialize the analytics SDK.
     * This method is async for React Native support (storage initialization).
     *
     * @param config - Configuration options
     */
    static async init(config?: HyperAnalyticsConfig): Promise<void> {
      if(this.didInit) return;

      // Initialize platform abstraction layer
      await initializePlatform();

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

      // Start periodic auto-flush if configured
      if (config?.flushInterval !== undefined && config.flushInterval > 0) {
        Configuration.setFlushInterval(config.flushInterval);
        this.startPeriodicFlush(config.flushInterval);
      }

      // Initialize anonymous ID (async for React Native)
      await AnonymousId.initialize();

      // Restore pending events from previous session (React Native only)
      await Batcher.restoreBatch();

      // Set up lifecycle event handlers
      this.setupLifecycleHandlers();

      this.didInit = true;
    }

    /**
     * Set up platform-specific lifecycle handlers.
     */
    private static setupLifecycleHandlers(): void {
      if (!isPlatformInitialized()) return;

      const platform = getPlatform();
      const lifecycle = getLifecycle();

      if (platform === 'react-native') {
        // React Native: Flush on background, prepare for termination
        const cleanupBackground = lifecycle.onBackground(async () => {
          this.stopPeriodicFlush();
          // Flush events
          await this.flush(true);
          // Persist any remaining events for next launch
          await Batcher.persistBatch();
        });

        const cleanupForeground = lifecycle.onForeground(() => {
          if (Configuration.flushInterval) {
            this.startPeriodicFlush(Configuration.flushInterval);
          }
        });

        this.cleanupFns.push(cleanupBackground, cleanupForeground);
      } else if (platform === 'browser') {
        // Browser: visibilitychange for page unload
        const cleanupBackground = lifecycle.onBackground(() => {
          this.stopPeriodicFlush();
          this.flush(true).catch(err => console.error('Flush on hidden error:', err));
        });

        this.cleanupFns.push(cleanupBackground);
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
     * @param useBeacon - Use unreliable delivery for page unload/app background
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

      if(containsNonPrimitives(props)) throw new Error("only primitives are allowed as properties");

      // Create session details (empty for now)
      const sessionDetails: Record<string, JSONSerializable> = {};

      // Get super properties
      const superProperties = SuperProperties.getSuperProperties();

      // Merge all properties (user properties take precedence over super properties)
      const mergedProperties: Record<string, JSONSerializable> = {
        ...superProperties,
        ...(typeof props === 'object' && props !== null && !Array.isArray(props) ? props : {}),
        ...sessionDetails,
      };

      // Create event object with current user
      const event: Event = {
        eventName: eventName.toString(),
        properties: mergedProperties,
        user: this.currentUser,
        anon_id: AnonymousId.getOrCreate(),
        eventId: this.generateEventId(),
        at: Date.now(),
      };

      // Add to batch
      Batcher.addToBatch(event);
    }

    /**
     * Generate a unique event ID.
     */
    private static generateEventId(): string {
      if (typeof crypto !== 'undefined' && crypto.randomUUID) {
        return crypto.randomUUID();
      }
      // Fallback UUID generation
      return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
        const r = (Math.random() * 16) | 0;
        const v = c === 'x' ? r : (r & 0x3) | 0x8;
        return v.toString(16);
      });
    }

    /**
     * Reset the SDK state. Useful for testing or logging out.
     */
    static async reset(): Promise<void> {
      this.stopPeriodicFlush();
      this.cleanupFns.forEach(fn => fn());
      this.cleanupFns = [];
      this.currentUser = null;
      this.didInit = false;
      Batcher.reset();
      AnonymousId.resetSync();
      resetPlatform();
    }
}

export { HyperAnalytics };