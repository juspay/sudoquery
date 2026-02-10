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
export type JSONSerializable =
  | string
  | number
  | boolean
  | null
  | JSONSerializable[]
  | { [key: string]: JSONSerializable };

export type Event = {
  eventName: string;
  properties: JSONSerializable;
  user: string | null;
  group: string | null;
  anon_id: string; // Anonymous ID for session tracking
  eventId: string; // Unique event ID for deduplication
  at: number; // Unix timestamp in milliseconds (UTC)
};

/**
 * Session data sent with each batch payload to the server.
 * Contains information about the user session and device/browser context.
 */
export type SessionData = {
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
export type ClientEvent = {
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
export type BatchPayload = {
  session: SessionData;
  events: ClientEvent[];
};
