import { describe, expect, it } from 'vitest';
import {
  DEFAULT_RANGE,
  QUICK_RANGES,
  formatBucket,
  intervalMs,
  rangeLabel,
  resolveRange,
  resolveTime,
} from './timeRange';

const now = new Date('2026-09-01T12:00:00.000Z');

describe('resolveTime', () => {
  it('resolves relative tokens against the given moment', () => {
    const cases: Array<[string, string]> = [
      ['now', '2026-09-01T12:00:00.000Z'],
      ['now-30s', '2026-09-01T11:59:30.000Z'],
      ['now-15m', '2026-09-01T11:45:00.000Z'],
      ['now-4h', '2026-09-01T08:00:00.000Z'],
      ['now-7d', '2026-08-25T12:00:00.000Z'],
      ['now-1w', '2026-08-25T12:00:00.000Z'],
      [' now-15m ', '2026-09-01T11:45:00.000Z'],
    ];

    for (const [token, expected] of cases) {
      expect(resolveTime(token, now)?.toISOString(), token).toBe(expected);
    }
  });

  it('reads absolute timestamps as they are', () => {
    expect(resolveTime('2026-08-31T23:00:00.000Z', now)?.toISOString()).toBe(
      '2026-08-31T23:00:00.000Z',
    );
  });

  it('rejects anything else', () => {
    for (const token of ['', 'yesterday', 'now-', 'now-5x', 'now+5m', 'now-m']) {
      expect(resolveTime(token, now), token).toBeNull();
    }
  });
});

describe('resolveRange', () => {
  it('resolves both ends', () => {
    expect(resolveRange(DEFAULT_RANGE, now)).toEqual({
      from: new Date('2026-09-01T11:45:00.000Z'),
      to: now,
    });
  });

  it('refuses an empty, backwards or unreadable range', () => {
    expect(resolveRange({ from: 'now', to: 'now' }, now)).toBeNull();
    expect(resolveRange({ from: 'now', to: 'now-1h' }, now)).toBeNull();
    expect(resolveRange({ from: 'nonsense', to: 'now' }, now)).toBeNull();
  });
});

describe('rangeLabel', () => {
  it('names ranges that end now', () => {
    expect(rangeLabel({ from: 'now-15m', to: 'now' })).toBe('Last 15 minutes');
    expect(rangeLabel({ from: 'now-7d', to: 'now' })).toBe('Last 7 days');
    expect(rangeLabel({ from: 'now-1h', to: 'now' })).toBe('Last 1 hour');
    expect(rangeLabel({ from: 'now-2h', to: 'now' })).toBe('Last 2 hours');
  });

  it('gives every quick range the label it is listed under', () => {
    for (const { label, range } of QUICK_RANGES) {
      expect(rangeLabel(range)).toBe(label);
    }
  });

  it('shows both ends of any other range', () => {
    expect(rangeLabel({ from: 'now-2h', to: 'now-1h' })).toBe('now-2h → now-1h');
    expect(rangeLabel({ from: '2026-09-01T10:00:00', to: '2026-09-01T11:00:00' })).toBe(
      'Sep 1, 2026 10:00:00 → Sep 1, 2026 11:00:00',
    );
  });
});

describe('intervalMs', () => {
  it('reads the intervals the server uses', () => {
    expect(intervalMs('30s')).toBe(30_000);
    expect(intervalMs('5m')).toBe(300_000);
    expect(intervalMs('12h')).toBe(43_200_000);
    expect(intervalMs('365d')).toBe(31_536_000_000);
    expect(intervalMs('soon')).toBeNull();
  });
});

describe('formatBucket', () => {
  const time = new Date(2026, 8, 30, 18, 30, 15);
  const hour = 3_600_000;

  it('shows the time of day within a single day', () => {
    expect(formatBucket(time, '30s', hour)).toBe('18:30:15');
    expect(formatBucket(time, '5m', 12 * hour)).toBe('18:30');
  });

  it('adds the date once the chart spans more than a day', () => {
    expect(formatBucket(time, '12h', 30 * 24 * hour)).toBe('Sep 30 18:30');
  });

  it('shows only the date for day-wide buckets', () => {
    expect(formatBucket(time, '1d', 90 * 24 * hour)).toBe('Sep 30');
  });
});
