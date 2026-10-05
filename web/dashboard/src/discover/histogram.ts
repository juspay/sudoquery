import { intervalMs } from './timeRange';

export interface Bucket {
  /** Start of the bucket, ISO. */
  time: string;
  count: number;
}

export interface BucketSelection {
  from: Date;
  /** End of the last selected bucket. */
  to: Date;
  count: number;
}

/**
 * The time covered by the buckets between two indices, in either order: from
 * the start of the earlier one to the end of the later one.
 */
export function bucketSelection(
  buckets: Bucket[],
  interval: string,
  anchor: number,
  current: number,
): BucketSelection | null {
  const first = Math.min(anchor, current);
  const last = Math.max(anchor, current);
  if (first < 0 || last >= buckets.length) return null;

  const from = new Date(buckets[first].time);
  const end = new Date(buckets[last].time);
  const to = new Date(end.getTime() + (intervalMs(interval) ?? 0));
  const count = buckets.slice(first, last + 1).reduce((sum, bucket) => sum + bucket.count, 0);
  return { from, to, count };
}

/** Horizontal extent of a drawn bar. */
export interface BarSpan {
  index: number;
  x: number;
  width: number;
}

/**
 * The bar under `x`, or the nearest one when `x` falls in a gap or past
 * either end, so a drag never loses track of where it is.
 */
export function nearestBar(bars: readonly BarSpan[], x: number): number | null {
  let best: { index: number; distance: number } | null = null;
  for (const bar of bars) {
    const distance = x < bar.x ? bar.x - x : x > bar.x + bar.width ? x - bar.x - bar.width : 0;
    if (best === null || distance < best.distance) {
      best = { index: bar.index, distance };
    }
  }
  return best?.index ?? null;
}
