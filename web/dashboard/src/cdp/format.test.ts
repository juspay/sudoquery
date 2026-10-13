import { describe, expect, it } from 'vitest';
import { formatCount, formatEpoch, formatMetric, formatMetricValue } from './format';

describe('formatMetric', () => {
  it('groups numbers and marks absence', () => {
    expect(formatMetric('4999')).toBe((4999).toLocaleString());
    expect(formatMetric(null)).toBe('—');
    expect(formatMetric('')).toBe('—');
    expect(formatMetric('blue')).toBe('blue');
  });
});

describe('formatEpoch', () => {
  it('reads epoch seconds', () => {
    expect(formatEpoch('0')).toBe(
      new Date(0).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }),
    );
    expect(formatEpoch(null)).toBe('—');
  });
});

describe('formatMetricValue', () => {
  it('treats *_at metrics as dates', () => {
    expect(formatMetricValue('last_order_at', '0')).toBe(formatEpoch('0'));
    expect(formatMetricValue('total_spend', '1200')).toBe(formatMetric('1200'));
  });
});

describe('formatCount', () => {
  it('formats counts', () => {
    expect(formatCount(1200)).toBe((1200).toLocaleString());
    expect(formatCount(null)).toBe('—');
  });
});
