import { describe, expect, it } from 'vitest';
import { DEFAULT_STATE, parseState, serializeState } from './urlState';
import type { DiscoverState } from './types';

describe('url state', () => {
  it('writes nothing for the default search', () => {
    expect(serializeState(DEFAULT_STATE).toString()).toBe('');
    expect(parseState(new URLSearchParams())).toEqual(DEFAULT_STATE);
  });

  it('round-trips a full search', () => {
    const state: DiscoverState = {
      query: 'name:checkout_viewed and properties.plan:"pro, annual"',
      language: 'lucene',
      time: { from: '2026-09-01T10:00:00.000Z', to: 'now' },
      filters: [
        { field: 'name', operator: 'is', value: 'click', negate: false, disabled: false },
        { field: 'authenticated', operator: 'is', value: true, negate: true, disabled: false },
        { field: 'source', operator: 'is_one_of', values: ['web', 'ios'], negate: false, disabled: true },
        { field: 'actor_id', operator: 'exists', negate: true, disabled: false },
        { field: 'properties.amount', operator: 'range', gte: 10, lt: 20, negate: false, disabled: false },
      ],
      columns: ['name', 'properties.plan'],
      order: 'asc',
      refresh: '30s',
    };

    const params = new URLSearchParams(serializeState(state).toString());

    expect(parseState(params)).toEqual(state);
  });

  it('falls back to defaults for unknown values', () => {
    const params = new URLSearchParams('lang=sql&sort=sideways&from=now-1h&refresh=1s');

    expect(parseState(params)).toEqual(DEFAULT_STATE);
  });

  it('drops malformed filters instead of failing', () => {
    const filters = [
      { field: 'name', operator: 'is', value: 'click' },
      { field: '', operator: 'is', value: 'x' },
      { field: 'name', operator: 'like', value: 'x' },
      { field: 'name', operator: 'is' },
      { field: 'name', operator: 'is', value: { nested: true } },
      { field: 'name', operator: 'is_one_of', values: [] },
      { field: 'name', operator: 'range' },
      'not an object',
      null,
    ];
    const params = new URLSearchParams({ f: JSON.stringify(filters) });

    expect(parseState(params).filters).toEqual([
      { field: 'name', operator: 'is', value: 'click', negate: false, disabled: false },
    ]);
    expect(parseState(new URLSearchParams('f=%7Bnot-json')).filters).toEqual([]);
    expect(parseState(new URLSearchParams('f=%7B%7D')).filters).toEqual([]);
  });
});
