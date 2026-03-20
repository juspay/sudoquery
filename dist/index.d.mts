/**
 * Represents any value that can be safely serialized to JSON and deserialized back
 * without losing data or structure.
 *
 * This includes:
 * - Primitives: string, number, boolean, null
 * - Arrays of JSON-serializable values
 * - Objects with string keys and JSON-serializable values
 *
 * @example
 * const valid: JSONSerializable = {
 *   name: "test",
 *   count: 42,
 *   active: true,
 *   nested: { value: null },
 *   items: [1, 2, 3]
 * };
 */
type JSONSerializable = string | number | boolean | null | JSONSerializable[] | {
    [key: string]: JSONSerializable;
};

interface HyperAnalyticsConfig {
    flushInterval?: number;
    batchSize?: number;
    endpoint?: string;
    token?: string;
}
declare class HyperAnalytics {
    private static didInit;
    private static currentUser;
    private static flushTimer;
    static init(config?: HyperAnalyticsConfig): void;
    private static startPeriodicFlush;
    private static stopPeriodicFlush;
    /**
     * Check if the SDK has been initialized
     */
    static get isInitialized(): boolean;
    /**
     * Get the current batch size
     */
    static get batchSize(): number;
    /**
     * Get the current endpoint URL
     */
    static get endpoint(): string;
    /**
     * Add a property to super properties (included in all events)
     */
    static setSuperProperty(key: string, value: JSONSerializable): void;
    /**
     * Get all current super properties
     */
    static getSuperProperties(): Record<string, JSONSerializable>;
    /**
     * Clear all super properties
     */
    static clearSuperProperties(): void;
    /**
     * Flush all pending events to the server
     * @param useBeacon - Use navigator.sendBeacon for more reliable delivery during page unload
     */
    static flush(useBeacon?: boolean): Promise<void>;
    /**
     * Set the user ID for all subsequent events
     * @param userId - The user identifier
     */
    static setUser(userId: string): void;
    /**
     * Remove the current user ID (resets to null)
     */
    static removeUser(): void;
    /**
     * Get the current user ID
     * @returns The current user ID or null if not set
     */
    static getUser(): string | null;
    static track(eventName: string, properties?: JSONSerializable): void;
}

export { HyperAnalytics, type HyperAnalyticsConfig, type JSONSerializable };
