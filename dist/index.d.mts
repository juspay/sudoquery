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
type Event = {
    eventName: string;
    properties: JSONSerializable;
    user: string | null;
    group: string | null;
    anon_id: string;
    eventId: string;
    at: number;
};

declare function containsNonPrimitives(obj: JSONSerializable): boolean;

declare class HyperAnalytics {
    private static didInit;
    private static currentUser;
    private static currentGroup;
    static init(): void;
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

declare class Configuration {
    private static _batchSize;
    static get batchSize(): number;
    static set batchSize(value: number);
}

declare class SuperProperties {
    private static properties;
    static addToSuperProperties(key: string, value: JSONSerializable): void;
    static getSuperProperties(): Record<string, JSONSerializable>;
    static clearSuperProperties(): void;
}

declare function flush(useBeacon?: boolean): Promise<void>;

declare class Pusher {
    private static _isUploadInProgress;
    private static endpoint;
    static setEndpoint(url: string): void;
    /**
     * Transform internal Event array to BatchPayload format
     */
    private static transformBatch;
    static pushLogs(useBeacon?: boolean): Promise<Event[] | null>;
    private static sendWithBeacon;
    private static sendNormally;
    static startScheduler(time: number): Promise<void>;
}

declare class Batcher {
    private static batches;
    private static _currentBatchToUpload;
    private static currentAccumilatingBatch;
    static addToBatch(event: Event): void;
    static fetchBatchToUpload(): (Event[] | null);
    static addNewBatch(): void;
    static setMarkLastBatchUploaded(): number;
    /**
     * Reset all internal state. Useful for testing.
     */
    static reset(): void;
}

declare const add: (a: number, b: number) => number;
declare const logMessage: (msg: string) => void;

export { Batcher, Configuration, HyperAnalytics, type JSONSerializable, Pusher, SuperProperties, add, containsNonPrimitives, flush, logMessage };
