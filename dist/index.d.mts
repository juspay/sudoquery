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
    anon_id: string;
    eventId: string;
    at: number;
};
/**
 * Session data sent with each batch payload to the server.
 * Contains information about the user session and device/browser context.
 */
type SessionData = {
    device_type: string;
    platform: string;
    browser: string;
    country: string;
    city: string;
    ip_address: string | null;
    user_agent: string;
};
/**
 * Event structure expected by the server API.
 * Properties are JSON-stringified for transmission.
 */
type ClientEvent = {
    event_id: string;
    event_name: string;
    event_timestamp: number;
    user_id: string | null;
    anon_id: string | null;
    properties: string;
};
/**
 * Complete batch payload sent to the server.
 * Wraps session information and array of events.
 */
type BatchPayload = {
    session: SessionData;
    events: ClientEvent[];
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
    private static cleanupFns;
    /**
     * Initialize the analytics SDK.
     * This method is async for React Native support (storage initialization).
     *
     * @param config - Configuration options
     */
    static init(config?: HyperAnalyticsConfig): Promise<void>;
    /**
     * Set up platform-specific lifecycle handlers.
     */
    private static setupLifecycleHandlers;
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
     * @param useBeacon - Use unreliable delivery for page unload/app background
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
    /**
     * Generate a unique event ID.
     */
    private static generateEventId;
    /**
     * Reset the SDK state. Useful for testing or logging out.
     */
    static reset(): Promise<void>;
}

/**
 * Platform abstraction layer types.
 * Defines interfaces that each platform must implement.
 */
/**
 * Storage adapter for persistent data.
 * Used for anonymous ID and pending events.
 */
interface StorageAdapter {
    getItem(key: string): Promise<string | null>;
    setItem(key: string, value: string): Promise<void>;
    removeItem(key: string): Promise<void>;
}
/**
 * Lifecycle adapter for app state changes.
 * Handles background, foreground, and termination events.
 */
interface LifecycleAdapter {
    /** Register callback for when app goes to background */
    onBackground(callback: () => void): () => void;
    /** Register callback for when app comes to foreground */
    onForeground(callback: () => void): () => void;
    /** Register callback for app termination (best-effort) */
    onTerminate(callback: () => void): () => void;
}
/**
 * Network adapter for HTTP requests.
 * Handles normal and unreliable (page unload/terminate) delivery.
 */
interface NetworkAdapter {
    /** Send request with full error handling */
    send(url: string, payload: unknown, headers: Record<string, string>): Promise<boolean>;
    /** Send request without waiting for response (fire-and-forget) */
    sendUnreliable(url: string, payload: unknown, headers?: Record<string, string>): boolean;
}
/**
 * Device info adapter for platform detection.
 * Provides device and OS information.
 */
interface DeviceInfoAdapter {
    /** Get device type: 'mobile', 'tablet', 'desktop', etc. */
    getDeviceType(): Promise<string>;
    /** Get platform name: 'iOS', 'Android', 'Windows', etc. */
    getPlatform(): Promise<string>;
    /** Get OS version */
    getOSVersion(): Promise<string>;
    /** Get app version (mobile only) */
    getAppVersion(): Promise<string>;
    /** Get user agent string */
    getUserAgent(): string;
}
/**
 * Platform type identifier.
 */
type PlatformType = 'browser' | 'react-native' | 'node';

/**
 * Platform abstraction layer.
 * Detects runtime environment and provides appropriate adapters.
 */

/**
 * Detect the current runtime platform.
 */
declare function detectPlatform(): PlatformType;
/**
 * Get the detected platform (throws if not initialized).
 */
declare function getPlatform(): PlatformType;
/**
 * Check if platform has been initialized.
 */
declare function isPlatformInitialized(): boolean;
/**
 * Reset platform state. Used for testing.
 */
declare function resetPlatform(): void;

export { type BatchPayload, type ClientEvent, type DeviceInfoAdapter, type Event, HyperAnalytics, type HyperAnalyticsConfig, type JSONSerializable, type LifecycleAdapter, type NetworkAdapter, type PlatformType, type SessionData, type StorageAdapter, detectPlatform, getPlatform, isPlatformInitialized, resetPlatform };
