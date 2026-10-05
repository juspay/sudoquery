import { describe, expect, it } from 'vitest';
import { REFRESH_INTERVALS, parseRefreshInterval } from './autoRefresh';
import { intervalMs } from './timeRange';

describe('parseRefreshInterval', () => {
  it('accepts the offered intervals', () => {
    for (const { value } of REFRESH_INTERVALS) {
      expect(parseRefreshInterval(value)).toBe(value);
    }
  });

  it('turns anything else off, so a URL cannot ask for a refresh every millisecond', () => {
    for (const value of [null, '', '1s', '0s', '30', 'soon', '2m']) {
      expect(parseRefreshInterval(value), String(value)).toBeNull();
    }
  });

  it('only offers intervals of five seconds or more', () => {
    for (const { value } of REFRESH_INTERVALS) {
      expect(intervalMs(value)).toBeGreaterThanOrEqual(5_000);
    }
  });
});
