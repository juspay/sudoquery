import { describe, expect, it } from 'vitest';
import { EVENT_FIELDS, discoverFields, flattenEvent, toIndexPattern } from './fields';
import { topValues } from './topValues';

const event = {
  id: '1',
  name: 'checkout_viewed',
  occured_at: '2026-09-01T10:00:00Z',
  authenticated: true,
  properties: { plan: 'pro', cart: { coupon: 'WELCOME', items: [1, 2] }, empty: {} },
  system_properties: { geo: { country: 'IN' } },
};

describe('flattenEvent', () => {
  it('uses dotted names and keeps arrays and empty objects as values', () => {
    expect(flattenEvent(event)).toEqual({
      id: '1',
      name: 'checkout_viewed',
      occured_at: '2026-09-01T10:00:00Z',
      authenticated: true,
      'properties.plan': 'pro',
      'properties.cart.coupon': 'WELCOME',
      'properties.cart.items': [1, 2],
      'properties.empty': {},
      'system_properties.geo.country': 'IN',
    });
  });
});

describe('discoverFields', () => {
  it('lists the known event fields even with no events loaded', () => {
    expect(discoverFields([])).toEqual(EVENT_FIELDS);
  });

  it('adds the fields found in events, sorted, and not aggregatable', () => {
    const fields = discoverFields([event, { properties: { amount: 12, plan: 'free' } }]);
    const extra = fields.slice(EVENT_FIELDS.length);

    expect(extra).toEqual([
      { name: 'properties.amount', type: 'number', facetable: false },
      { name: 'properties.cart.coupon', type: 'string', facetable: false },
      { name: 'properties.cart.items', type: 'object', facetable: false },
      { name: 'properties.empty', type: 'object', facetable: false },
      { name: 'properties.plan', type: 'string', facetable: false },
    ]);
  });

  it('marks exactly the fields the server can aggregate', () => {
    const facetable = EVENT_FIELDS.filter((field) => field.facetable).map((field) => field.name);

    // Must match FACET_FIELDS in crates/dashboard-server/src/opensearch/query.rs.
    expect(facetable.sort()).toEqual(
      [
        'name',
        'source',
        'session_id',
        'anon_id',
        'actor_id',
        'correlation_id',
        'trace_id',
        'authenticated',
        'envelop_version',
        'system_properties.geo.country',
        'system_properties.timezone',
      ].sort(),
    );
  });
});

describe('toIndexPattern', () => {
  it('tells the DQL parser which fields are dates', () => {
    const dates = toIndexPattern(EVENT_FIELDS)
      .fields.filter((field) => field.type === 'date')
      .map((field) => field.name);

    expect(dates).toEqual(['occured_at', 'arrived_at', '@timestamp']);
  });
});

describe('topValues', () => {
  const docs = [
    { name: 'click', properties: { plan: 'pro', n: 1 } },
    { name: 'click', properties: { plan: 'pro', n: '1' } },
    { name: 'view', properties: { plan: 'free' } },
    { name: 'view' },
    { name: 'view', properties: { plan: null } },
  ];

  it('counts values among the events that have the field', () => {
    expect(topValues(docs, 'properties.plan')).toEqual({
      withField: 3,
      sampled: 5,
      buckets: [
        { value: 'pro', label: 'pro', count: 2 },
        { value: 'free', label: 'free', count: 1 },
      ],
    });
  });

  it('keeps values of different types apart', () => {
    expect(topValues(docs, 'properties.n').buckets).toEqual([
      { value: 1, label: '1', count: 1 },
      { value: '1', label: '1', count: 1 },
    ]);
  });

  it('returns the most frequent first, up to the limit', () => {
    expect(topValues(docs, 'name', 1).buckets).toEqual([{ value: 'view', label: 'view', count: 3 }]);
  });

  it('is empty for a field no event has', () => {
    expect(topValues(docs, 'properties.missing')).toEqual({ withField: 0, sampled: 5, buckets: [] });
  });
});
