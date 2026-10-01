import { flattenEvent } from './fields';
import type { EventDoc } from './types';

interface ValueCount {
  /** The value as found in the events; objects and arrays as JSON text. */
  value: unknown;
  label: string;
  count: number;
}

export interface TopValues {
  /** Events in the sample that have the field. */
  withField: number;
  /** Events in the sample. */
  sampled: number;
  buckets: ValueCount[];
}

export function valueLabel(value: unknown): string {
  if (value === null || value === undefined) return '—';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

/**
 * The most frequent values of `field` among `docs`. This is a sample: it only
 * sees the events loaded in the table, unlike the server's facet counts.
 */
export function topValues(docs: EventDoc[], field: string, limit = 5): TopValues {
  const counts = new Map<string, ValueCount>();
  let withField = 0;
  for (const doc of docs) {
    const flat = flattenEvent(doc);
    if (!(field in flat)) continue;
    const value = flat[field];
    if (value === null || value === undefined) continue;
    withField += 1;
    const label = valueLabel(value);
    const key = `${typeof value}:${label}`;
    const seen = counts.get(key);
    if (seen) {
      seen.count += 1;
    } else {
      counts.set(key, { value, label, count: 1 });
    }
  }
  const buckets = [...counts.values()]
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label))
    .slice(0, limit);
  return { withField, sampled: docs.length, buckets };
}
