import { format } from 'date-fns';
import type { ResolvedRange, TimeRange } from './types';

const UNIT_MS: Record<string, number> = {
  s: 1_000,
  m: 60_000,
  h: 3_600_000,
  d: 86_400_000,
  w: 604_800_000,
};

const RELATIVE = /^now(?:-(\d+)([smhdw]))?$/;

export const QUICK_RANGES: Array<{ label: string; range: TimeRange }> = [
  { label: 'Last 15 minutes', range: { from: 'now-15m', to: 'now' } },
  { label: 'Last 30 minutes', range: { from: 'now-30m', to: 'now' } },
  { label: 'Last 1 hour', range: { from: 'now-1h', to: 'now' } },
  { label: 'Last 4 hours', range: { from: 'now-4h', to: 'now' } },
  { label: 'Last 24 hours', range: { from: 'now-24h', to: 'now' } },
  { label: 'Last 7 days', range: { from: 'now-7d', to: 'now' } },
  { label: 'Last 30 days', range: { from: 'now-30d', to: 'now' } },
  { label: 'Last 90 days', range: { from: 'now-90d', to: 'now' } },
];

export const DEFAULT_RANGE: TimeRange = QUICK_RANGES[0].range;

/** Resolves one end of a range: `now`, `now-15m` or an ISO timestamp. */
export function resolveTime(token: string, now: Date): Date | null {
  const relative = RELATIVE.exec(token.trim());
  if (relative) {
    const [, amount, unit] = relative;
    const offset = amount ? Number(amount) * UNIT_MS[unit] : 0;
    return new Date(now.getTime() - offset);
  }
  const absolute = new Date(token);
  return Number.isNaN(absolute.getTime()) ? null : absolute;
}

/** `null` when either end is unreadable or the range is empty. */
export function resolveRange(range: TimeRange, now: Date): ResolvedRange | null {
  const from = resolveTime(range.from, now);
  const to = resolveTime(range.to, now);
  if (!from || !to || from.getTime() >= to.getTime()) {
    return null;
  }
  return { from, to };
}

export function formatTimestamp(date: Date): string {
  return format(date, 'MMM d, yyyy HH:mm:ss.SSS');
}

function formatEnd(token: string): string {
  if (RELATIVE.test(token.trim())) return token.trim();
  const date = new Date(token);
  return Number.isNaN(date.getTime()) ? token : format(date, 'MMM d, yyyy HH:mm:ss');
}

const UNIT_NAME: Record<string, string> = {
  s: 'second',
  m: 'minute',
  h: 'hour',
  d: 'day',
  w: 'week',
};

/** "Last 15 minutes" for a range that ends now, otherwise "from → to". */
export function rangeLabel(range: TimeRange): string {
  const from = RELATIVE.exec(range.from.trim());
  if (range.to.trim() === 'now' && from?.[1]) {
    const amount = Number(from[1]);
    return `Last ${amount} ${UNIT_NAME[from[2]]}${amount === 1 ? '' : 's'}`;
  }
  return `${formatEnd(range.from)} → ${formatEnd(range.to)}`;
}

/**
 * Whether the range ends at a fixed moment. Refreshing such a range shows
 * nothing new; one ending at `now` (or `now-1h`) moves forward instead.
 */
export function endsAtFixedTime(range: TimeRange): boolean {
  return !RELATIVE.test(range.to.trim());
}

/** Milliseconds in a histogram interval such as `30s`, `5m` or `1d`. */
export function intervalMs(interval: string): number | null {
  const match = /^(\d+)([smhdw])$/.exec(interval);
  return match ? Number(match[1]) * UNIT_MS[match[2]] : null;
}

/**
 * Axis label for a histogram bucket, as coarse as its interval allows. The
 * date is included once the chart covers more than a day, or every label
 * of 12h buckets would read the same few times.
 */
export function formatBucket(time: Date, interval: string, spanMs: number): string {
  const width = intervalMs(interval) ?? 0;
  if (width >= UNIT_MS.d) return format(time, 'MMM d');
  if (spanMs > UNIT_MS.d) return format(time, 'MMM d HH:mm');
  if (width >= UNIT_MS.m) return format(time, 'HH:mm');
  return format(time, 'HH:mm:ss');
}
