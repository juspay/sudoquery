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
type EnvelopVersion = "1.0";
type Geo = {
    country: string | null;
};
/**
 * System properties understood by the collector.
 */
type SystemProperties = {
    geo: Geo | null;
    timezone: string | null;
};
/**
 * Event structure accepted by the collector.
 *
 * Field names intentionally match the Rust collector schema, including
 * `envelop_version` and `occured_at`.
 */
type Event = {
    envelop_version: EnvelopVersion;
    id: string;
    name: string;
    org_id: string;
    project_id: string;
    session_id: string | null;
    anon_id: string;
    actor_id: string | null;
    source: string | null;
    occured_at: string;
    properties: JSONSerializable | null;
    correlation_id: string | null;
    trace_id: string | null;
    system_properties: SystemProperties | null;
};
/**
 * Complete batch payload sent to the collector.
 */
type BatchPayload = {
    events: Event[];
    system_properties: SystemProperties | null;
};

interface SudoQueryConfig {
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
declare class SudoQuery {
    private static didInit;
    private static currentUser;
    private static flushTimer;
    static init(config?: SudoQueryConfig): void;
    private static startPeriodicFlush;
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
     * @param useBeacon - Use fetch keepalive for more reliable delivery during page unload
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

export { type BatchPayload, type EnvelopVersion, type Event, type Geo, type JSONSerializable, SudoQuery, type SudoQueryConfig, type SystemProperties };
