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

export type EnvelopVersion = "1.0";

export type Geo = {
  country: string | null;
};

/**
 * System properties understood by the collector.
 */
export type SystemProperties = {
  geo: Geo | null;
  timezone: string | null;
};

/**
 * Event structure accepted by the collector.
 *
 * Field names intentionally match the Rust collector schema, including
 * `envelop_version` and `occured_at`.
 */
export type Event = {
  envelop_version: EnvelopVersion;
  id: string;
  name: string;
  org_id: string;
  proj_id: string | null;
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
export type BatchPayload = {
  events: Event[];
  system_properties: SystemProperties | null;
};
