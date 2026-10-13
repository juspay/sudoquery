/** Display formatting for CDP values (exact decimal strings, epoch seconds). */

/** `"4999"` → `"4,999"`; non-numeric strings pass through; null → em dash. */
export function formatMetric(value: string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—';
  const n = Number(value);
  if (!Number.isFinite(n)) return value;
  return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

/** Unix epoch seconds (as a string) → a short local date. */
export function formatEpoch(value: string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—';
  const seconds = Number(value);
  if (!Number.isFinite(seconds)) return value;
  return new Date(seconds * 1000).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/** An RFC 3339 timestamp → a short local date and time. */
export function formatTimestamp(value: string | null | undefined): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** Metric display: timestamp metrics (`*_at`) as dates, the rest as numbers. */
export function formatMetricValue(metric: string, value: string | null | undefined): string {
  return metric.endsWith('_at') ? formatEpoch(value) : formatMetric(value);
}

export function formatCount(n: number | null | undefined): string {
  return n === null || n === undefined ? '—' : n.toLocaleString();
}
