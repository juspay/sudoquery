import { describe, expect, it } from 'vitest';
import {
  describePredicate,
  emptyCondition,
  metricChoices,
  opsFor,
  slugify,
  toPredicate,
  type Condition,
} from './predicate';
import type { MetricDefinition } from '../services/cdpService';

const cond = (over: Partial<Condition>): Condition => ({ ...emptyCondition(), ...over });

describe('toPredicate', () => {
  it('makes a single condition a bare leaf', () => {
    expect(
      toPredicate({ match: 'all', conditions: [cond({ metric: 'total_spend', op: 'gte', value: '500' })] }),
    ).toEqual({ predicate: { metric: 'total_spend', op: 'gte', value: 500 } });
  });

  it('groups several conditions under all / any', () => {
    const conditions = [
      cond({ metric: 'total_spend', op: 'gte', value: '500' }),
      cond({ metric: 'last_order_at', op: 'within_days', value: '30' }),
    ];
    expect(toPredicate({ match: 'any', conditions })).toEqual({
      predicate: {
        any: [
          { metric: 'total_spend', op: 'gte', value: 500 },
          { metric: 'last_order_at', op: 'within_days', value: 30 },
        ],
      },
    });
  });

  it('leaves the value out for exists / missing', () => {
    expect(
      toPredicate({ match: 'all', conditions: [cond({ metric: 'last_order_at', op: 'missing', value: '7' })] }),
    ).toEqual({ predicate: { metric: 'last_order_at', op: 'missing' } });
  });

  it('builds an inclusive range for between', () => {
    expect(
      toPredicate({
        match: 'all',
        conditions: [cond({ metric: 'total_spend', op: 'between', value: '1000', valueTo: '5000' })],
      }),
    ).toEqual({ predicate: { metric: 'total_spend', op: 'between', value: [1000, 5000] } });
  });

  it('names the first incomplete condition', () => {
    expect(toPredicate({ match: 'all', conditions: [] })).toEqual({ error: 'add at least one condition' });
    expect(
      toPredicate({
        match: 'all',
        conditions: [cond({ metric: 'a', value: '1' }), cond({ metric: 'b', value: 'x' })],
      }),
    ).toEqual({ error: 'condition 2: enter a number' });
    expect(toPredicate({ match: 'all', conditions: [cond({})] })).toEqual({
      error: 'condition 1: choose a metric',
    });
    expect(
      toPredicate({
        match: 'all',
        conditions: [cond({ metric: 'a', op: 'between', value: '9', valueTo: '1' })],
      }),
    ).toEqual({ error: 'condition 1: the range must go from low to high' });
    expect(
      toPredicate({ match: 'all', conditions: [cond({ metric: 'a', op: 'within_days', value: '1.5' })] }),
    ).toEqual({ error: 'condition 1: days must be a whole number above 0' });
  });
});

describe('opsFor', () => {
  it('offers day ops only for timestamp-like metrics', () => {
    expect(opsFor('property_number')).not.toContain('within_days');
    expect(opsFor('occurred_at_epoch')[0]).toBe('within_days');
    expect(opsFor('property_string')).toEqual(['within_days', 'beyond_days', 'exists', 'missing']);
  });
});

describe('describePredicate', () => {
  it('reads nested groups', () => {
    const names: Record<string, string> = { total_spend: 'Total Spend' };
    expect(
      describePredicate(
        {
          all: [
            { metric: 'total_spend', op: 'gte', value: 10000 },
            {
              any: [
                { metric: 'order_count', op: 'gte', value: 3 },
                { metric: 'last_order_at', op: 'within_days', value: 30 },
              ],
            },
            { not: { metric: 'refund_count', op: 'gt', value: 2 } },
          ],
        },
        (m) => names[m] ?? m,
      ),
    ).toBe(
      'Total Spend at least 10000 and (order_count at least 3 or last_order_at within the last 30 days) and not refund_count greater than 2',
    );
  });
});

describe('metricChoices', () => {
  it('lists each metric once, sorted by display name', () => {
    const def = (metric_name: string, event_name: string, display_name: string | null): MetricDefinition => ({
      workspace_id: '*',
      metric_name,
      event_name,
      value_kind: 'occurred_at_epoch',
      display_name,
    });
    const choices = metricChoices([
      def('last_seen_at', 'page_viewed', 'Last Seen At'),
      def('last_seen_at', 'order_completed', 'Last Seen At'),
      def('cart_adds', 'add_to_cart', null),
    ]);
    expect(choices.map((c) => c.metric_name)).toEqual(['cart_adds', 'last_seen_at']);
  });
});

describe('slugify', () => {
  it('makes a segment id from a name', () => {
    expect(slugify('  VIP Buyers (Q4)! ')).toBe('vip-buyers-q4');
    expect(slugify('Café clients')).toBe('cafe-clients');
  });
});
