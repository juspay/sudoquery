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

declare class HyperAnalytics {
    private static didInit;
    private static currentUser;
    private static currentGroup;
    static init(): void;
    /**
     * Set the batch size for event batching
     */
    static set batchSize(value: number);
    /**
     * Get the current batch size
     */
    static get batchSize(): number;
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
    /**
     * Set the group ID for all subsequent events
     * @param groupId - The group identifier
     */
    static setGroup(groupId: string): void;
    /**
     * Remove the current group ID (resets to null)
     */
    static removeGroup(): void;
    /**
     * Get the current group ID
     * @returns The current group ID or null if not set
     */
    static getGroup(): string | null;
    static track(eventName: String, properties: JSONSerializable): void;
}

export { HyperAnalytics };
