import { describe, expect, it } from 'vitest';
import { buildQuery, type Dsl } from './buildQuery';
import { EVENT_FIELDS } from './fields';
import type { FilterPill, QueryLanguage } from './types';

const range = { from: new Date('2026-09-01T10:00:00Z'), to: new Date('2026-09-01T11:00:00Z') };
const timeFilter = {
  range: {
    occured_at: {
      gte: '2026-09-01T10:00:00.000Z',
      lte: '2026-09-01T11:00:00.000Z',
      format: 'strict_date_optional_time',
    },
  },
};
const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;

function build(query: string, filters: FilterPill[] = [], language: QueryLanguage = 'dql') {
  return buildQuery({ query, language, filters }, range, EVENT_FIELDS);
}

function bool(query: string, filters: FilterPill[] = [], language: QueryLanguage = 'dql') {
  const built = build(query, filters, language);
  if (!built.ok) throw new Error(built.error);
  return built.query.bool as { must: Dsl[]; filter: Dsl[]; should: Dsl[]; must_not: Dsl[] };
}

const pill = (overrides: Partial<FilterPill>): FilterPill => ({
  field: 'name',
  operator: 'is',
  value: 'click',
  negate: false,
  disabled: false,
  ...overrides,
});

describe('buildQuery', () => {
  it('matches everything in the time range when the query is empty', () => {
    expect(build('   ')).toEqual({
      ok: true,
      query: { bool: { must: [], filter: [timeFilter], should: [], must_not: [] } },
    });
  });

  it('puts the time range last in the root filter, where the server looks for it', () => {
    const { filter } = bool('name:click', [pill({ field: 'source', value: 'web' })]);

    expect(filter).toHaveLength(3);
    expect(filter[2]).toEqual(timeFilter);
  });

  it('leaves the time range out when none is given', () => {
    const built = buildQuery({ query: '', language: 'dql', filters: [] }, undefined, EVENT_FIELDS);

    expect(built).toEqual({
      ok: true,
      query: { bool: { must: [], filter: [], should: [], must_not: [] } },
    });
  });

  it('compiles DQL into a non-scoring filter', () => {
    const { must, filter } = bool('name:checkout_viewed and not actor_id:*');

    expect(must).toEqual([]);
    expect(filter[0]).toEqual({
      bool: {
        filter: [
          { bool: { should: [{ match: { name: 'checkout_viewed' } }], minimum_should_match: 1 } },
          {
            bool: {
              must_not: {
                bool: { should: [{ exists: { field: 'actor_id' } }], minimum_should_match: 1 },
              },
            },
          },
        ],
      },
    });
  });

  it('treats the event time as a date, in the browser time zone', () => {
    const { filter } = bool('occured_at >= "2026-09-01"');

    expect(filter[0]).toEqual({
      bool: {
        should: [{ range: { occured_at: { gte: '2026-09-01', time_zone: timeZone } } }],
        minimum_should_match: 1,
      },
    });
  });

  it('passes fields it has never seen through as typed', () => {
    const { filter } = bool('properties.plan:pro');

    expect(filter[0]).toEqual({
      bool: { should: [{ match: { 'properties.plan': 'pro' } }], minimum_should_match: 1 },
    });
  });

  it('sends integers too large for JSON as text', () => {
    const built = build('properties.order_id:9007199254740993');

    expect(() => JSON.stringify(built)).not.toThrow();
    expect(JSON.stringify(built)).toContain('"9007199254740993"');
  });

  it('reports a DQL syntax error instead of a query', () => {
    expect(build('name:')).toEqual({
      ok: false,
      error: 'Expected whitespace, "{", "(", value but end of input found.',
    });
  });

  it('sends Lucene as a scoring query_string', () => {
    const { must, filter } = bool('name:checkout AND properties.plan:pro', [], 'lucene');

    expect(must).toEqual([
      {
        query_string: {
          query: 'name:checkout AND properties.plan:pro',
          analyze_wildcard: true,
          time_zone: timeZone,
        },
      },
    ]);
    expect(filter).toEqual([timeFilter]);
  });

  it('does not parse Lucene text as DQL', () => {
    expect(build('name:[a TO b]', [], 'lucene').ok).toBe(true);
  });

  it('adds pills as filters and negated pills as must_not', () => {
    const { filter, must_not: mustNot } = bool('', [
      pill({}),
      pill({ field: 'source', value: 'web', negate: true }),
      pill({ field: 'actor_id', operator: 'exists', value: undefined }),
    ]);

    expect(filter).toEqual([
      { match_phrase: { name: 'click' } },
      { exists: { field: 'actor_id' } },
      timeFilter,
    ]);
    expect(mustNot).toEqual([{ match_phrase: { source: 'web' } }]);
  });

  it('leaves disabled pills out', () => {
    const { filter, must_not: mustNot } = bool('', [
      pill({ disabled: true }),
      pill({ field: 'source', value: 'web', negate: true, disabled: true }),
    ]);

    expect(filter).toEqual([timeFilter]);
    expect(mustNot).toEqual([]);
  });
});
