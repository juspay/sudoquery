import { describe, expect, it } from 'vitest';
import { formatDuration, formatOffset, nameColors } from './format';

describe('formatDuration', () => {
  it('picks the unit that fits', () => {
    const cases: Array<[number, string]> = [
      [0, '0ms'],
      [850, '850ms'],
      [3_200, '3.2s'],
      [59_900, '59.9s'],
      [245_000, '4m 05s'],
      [7_620_000, '2h 07m'],
      [273_600_000, '3d 4h'],
    ];

    for (const [ms, expected] of cases) {
      expect(formatDuration(ms), String(ms)).toBe(expected);
    }
  });

  it('has no text for a missing duration', () => {
    expect(formatDuration(Number.NaN)).toBe('—');
    expect(formatDuration(-1)).toBe('—');
  });
});

describe('formatOffset', () => {
  it('is a duration since the session started', () => {
    expect(formatOffset(0)).toBe('+0ms');
    expect(formatOffset(3_200)).toBe('+3.2s');
  });
});

describe('nameColors', () => {
  it('gives each distinct name its own colour, in order of first appearance', () => {
    const colors = nameColors(['page_viewed', 'product_viewed', 'page_viewed', 'add_to_cart']);

    expect([...colors.keys()]).toEqual(['page_viewed', 'product_viewed', 'add_to_cart']);
    expect(new Set(colors.values()).size).toBe(3);
  });

  it('reuses colours only once there are more names than colours', () => {
    const names = Array.from({ length: 9 }, (_, index) => `event_${index}`);
    const colors = nameColors(names);

    expect(new Set([...colors.values()].slice(0, 8)).size).toBe(8);
    expect(colors.get('event_8')).toBe(colors.get('event_0'));
  });
});
