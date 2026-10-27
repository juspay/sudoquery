import type { IIndexPattern } from '../vendor/kuery';
import type { EventDoc, FieldDef, FieldType } from './types';

export const TIME_FIELD = 'occured_at';

/**
 * Fields the server can aggregate (`POST /facets`). Must match `FACET_FIELDS`
 * in crates/dashboard-server/src/opensearch/query.rs.
 */
const FACETABLE = new Set([
  'name',
  'source',
  'session_id',
  'anon_id',
  'actor_id',
  'correlation_id',
  'trace_id',
  'authenticated',
  'envelop_version',
  'system_properties.geo.country',
  'system_properties.timezone',
]);

/** The fields every event can carry, as the index template maps them. */
const EVENT_FIELD_TYPES: Array<[string, FieldType]> = [
  ['name', 'string'],
  [TIME_FIELD, 'date'],
  ['session_id', 'string'],
  ['anon_id', 'string'],
  ['actor_id', 'string'],
  ['source', 'string'],
  ['authenticated', 'boolean'],
  ['id', 'string'],
  ['arrived_at', 'date'],
  ['correlation_id', 'string'],
  ['trace_id', 'string'],
  ['envelop_version', 'string'],
  ['workspace_id', 'string'],
  ['system_properties.geo.country', 'string'],
  ['system_properties.timezone', 'string'],
  ['system_properties.ip_address', 'ip'],
];

export const EVENT_FIELDS: FieldDef[] = EVENT_FIELD_TYPES.map(([name, type]) => ({
  name,
  type,
  facetable: FACETABLE.has(name),
}));

const KNOWN = new Map(EVENT_FIELDS.map((field) => [field.name, field]));

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Flattens an event to dotted field names: `{ properties: { plan: 'pro' } }`
 * becomes `{ 'properties.plan': 'pro' }`. Arrays and empty objects stay values.
 */
export function flattenEvent(doc: EventDoc): Record<string, unknown> {
  const flat: Record<string, unknown> = {};
  const walk = (value: unknown, path: string) => {
    if (isPlainObject(value) && Object.keys(value).length > 0) {
      for (const [key, child] of Object.entries(value)) {
        walk(child, path ? `${path}.${key}` : key);
      }
    } else if (path) {
      flat[path] = value;
    }
  };
  walk(doc, '');
  return flat;
}

function typeOfValue(value: unknown): FieldType {
  switch (typeof value) {
    case 'string':
      return 'string';
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'object':
      return value === null ? 'unknown' : 'object';
    default:
      return 'unknown';
  }
}

/**
 * The known event fields followed by every other field seen in `docs`
 * (in practice `properties.*`), sorted by name.
 */
export function discoverFields(docs: EventDoc[]): FieldDef[] {
  const found = new Map<string, FieldType>();
  for (const doc of docs) {
    for (const [name, value] of Object.entries(flattenEvent(doc))) {
      if (KNOWN.has(name)) continue;
      const type = typeOfValue(value);
      const seen = found.get(name);
      if (seen === undefined || seen === 'unknown') {
        found.set(name, type);
      }
    }
  }
  const extra = [...found.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, type]) => ({ name, type, facetable: false }));
  return [...EVENT_FIELDS, ...extra];
}

/** What the DQL parser needs to know about fields: which ones are dates. */
export function toIndexPattern(fields: FieldDef[]): IIndexPattern {
  return {
    title: 'events',
    fields: [
      ...fields.map((field) => ({ name: field.name, type: field.type })),
      // Alias of the event time in the index template.
      { name: '@timestamp', type: 'date' },
    ],
  };
}
