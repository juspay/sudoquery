import { describe, expect, it } from 'vitest';
import { addExistsFilter, addValueFilter, filterLabel, filterToQuery } from './filters';
import type { FilterPill } from './types';

const base = { negate: false, disabled: false };

describe('filterToQuery', () => {
  it('builds one clause per operator', () => {
    const cases: Array<[FilterPill, unknown]> = [
      [{ ...base, field: 'name', operator: 'is', value: 'click' }, { match_phrase: { name: 'click' } }],
      [
        { ...base, field: 'authenticated', operator: 'is', value: true },
        { match_phrase: { authenticated: true } },
      ],
      [
        { ...base, field: 'name', operator: 'is_one_of', values: ['a', 'b'] },
        {
          bool: {
            should: [{ match_phrase: { name: 'a' } }, { match_phrase: { name: 'b' } }],
            minimum_should_match: 1,
          },
        },
      ],
      [{ ...base, field: 'actor_id', operator: 'exists' }, { exists: { field: 'actor_id' } }],
      [
        { ...base, field: 'properties.amount', operator: 'range', gte: 10, lt: 20 },
        { range: { 'properties.amount': { gte: 10, lt: 20 } } },
      ],
      [
        { ...base, field: 'properties.amount', operator: 'range', gte: 10 },
        { range: { 'properties.amount': { gte: 10 } } },
      ],
    ];

    for (const [pill, query] of cases) {
      expect(filterToQuery(pill)).toEqual(query);
    }
  });

  it('ignores negation, which the caller applies', () => {
    const pill: FilterPill = { field: 'name', operator: 'is', value: 'click', negate: true, disabled: false };

    expect(filterToQuery(pill)).toEqual({ match_phrase: { name: 'click' } });
  });
});

describe('filterLabel', () => {
  it('describes each kind of pill', () => {
    expect(filterLabel({ ...base, field: 'name', operator: 'is', value: 'click' })).toBe('name: click');
    expect(filterLabel({ ...base, field: 'name', operator: 'is_one_of', values: ['a', 'b'] })).toBe(
      'name: a, b',
    );
    expect(filterLabel({ ...base, field: 'actor_id', operator: 'exists' })).toBe('actor_id: exists');
    expect(filterLabel({ ...base, field: 'n', operator: 'range', gte: 1 })).toBe('n: 1 to *');
  });
});

describe('addValueFilter', () => {
  it('appends a pill', () => {
    expect(addValueFilter([], 'name', 'click', false)).toEqual([
      { field: 'name', operator: 'is', value: 'click', negate: false, disabled: false },
    ]);
  });

  it('flips an existing pill for the same field and value instead of contradicting it', () => {
    const filters = addValueFilter(addValueFilter([], 'name', 'click', false), 'source', 'web', false);

    const flipped = addValueFilter(filters, 'name', 'click', true);

    expect(flipped).toHaveLength(2);
    expect(flipped[0]).toMatchObject({ field: 'name', value: 'click', negate: true });
    expect(flipped[1]).toMatchObject({ field: 'source', negate: false });
  });

  it('keeps pills for other values of the same field', () => {
    const filters = addValueFilter(addValueFilter([], 'name', 'click', false), 'name', 'view', false);

    expect(filters.map((pill) => pill.value)).toEqual(['click', 'view']);
  });

  it('tells 1 from "1"', () => {
    const filters = addValueFilter(addValueFilter([], 'properties.n', 1, false), 'properties.n', '1', false);

    expect(filters).toHaveLength(2);
  });
});

describe('addExistsFilter', () => {
  it('keeps one exists pill per field', () => {
    const filters = addExistsFilter(addExistsFilter([], 'actor_id', false), 'actor_id', true);

    expect(filters).toEqual([
      { field: 'actor_id', operator: 'exists', negate: true, disabled: false },
    ]);
  });
});
