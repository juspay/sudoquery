import { describe, expect, it } from 'vitest';
import { bucketSelection, nearestBar } from './histogram';

const buckets = [
  { time: '2026-09-30T10:00:00.000Z', count: 3 },
  { time: '2026-09-30T10:05:00.000Z', count: 0 },
  { time: '2026-09-30T10:10:00.000Z', count: 7 },
  { time: '2026-09-30T10:15:00.000Z', count: 1 },
];

describe('bucketSelection', () => {
  it('runs from the start of the first bucket to the end of the last', () => {
    expect(bucketSelection(buckets, '5m', 1, 2)).toEqual({
      from: new Date('2026-09-30T10:05:00.000Z'),
      to: new Date('2026-09-30T10:15:00.000Z'),
      count: 7,
    });
  });

  it('works whichever way the drag went', () => {
    expect(bucketSelection(buckets, '5m', 3, 0)).toEqual(bucketSelection(buckets, '5m', 0, 3));
    expect(bucketSelection(buckets, '5m', 3, 0)?.count).toBe(11);
  });

  it('covers one bucket for a plain click', () => {
    expect(bucketSelection(buckets, '5m', 2, 2)).toEqual({
      from: new Date('2026-09-30T10:10:00.000Z'),
      to: new Date('2026-09-30T10:15:00.000Z'),
      count: 7,
    });
  });

  it('has nothing for indices outside the chart', () => {
    expect(bucketSelection(buckets, '5m', -1, 2)).toBeNull();
    expect(bucketSelection(buckets, '5m', 0, 4)).toBeNull();
  });
});

describe('nearestBar', () => {
  const bars = [
    { index: 0, x: 10, width: 8 },
    { index: 1, x: 20, width: 8 },
    { index: 2, x: 30, width: 8 },
  ];

  it('finds the bar under the pointer', () => {
    expect(nearestBar(bars, 24)).toBe(1);
    expect(nearestBar(bars, 30)).toBe(2);
  });

  it('snaps to the closest bar in a gap or past either end', () => {
    expect(nearestBar(bars, 19.5)).toBe(1);
    expect(nearestBar(bars, 18.5)).toBe(0);
    expect(nearestBar(bars, 0)).toBe(0);
    expect(nearestBar(bars, 500)).toBe(2);
  });

  it('has nothing to find on an empty chart', () => {
    expect(nearestBar([], 5)).toBeNull();
  });
});
