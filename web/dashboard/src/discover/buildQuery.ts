import { DQLSyntaxError, fromKueryExpression, toOpenSearchQuery } from '../vendor/kuery';
import { TIME_FIELD, toIndexPattern } from './fields';
import { filterToQuery } from './filters';
import type { DiscoverState, FieldDef, ResolvedRange } from './types';

export type Dsl = Record<string, unknown>;

export type BuiltQuery = { ok: true; query: Dsl } | { ok: false; error: string };

type QueryParts = Pick<DiscoverState, 'query' | 'language' | 'filters'>;

/**
 * The DQL parser turns integers beyond the safe range into BigInt, which
 * JSON cannot carry. OpenSearch reads a numeric string the same way.
 */
function stringifyBigInts(value: unknown): unknown {
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) return value.map(stringifyBigInts);
  if (typeof value === 'object' && value !== null) {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [key, stringifyBigInts(child)]),
    );
  }
  return value;
}

function dqlToDsl(query: string, fields: FieldDef[]): Dsl {
  const ast = fromKueryExpression(query, { allowLeadingWildcards: true });
  const dsl = toOpenSearchQuery(ast, toIndexPattern(fields), { dateFormatTZ: 'Browser' });
  return stringifyBigInts(dsl) as Dsl;
}

function luceneToDsl(query: string): Dsl {
  return {
    query_string: {
      query,
      analyze_wildcard: true,
      time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    },
  };
}

function timeFilter(range: ResolvedRange): Dsl {
  return {
    range: {
      [TIME_FIELD]: {
        gte: range.from.toISOString(),
        lte: range.to.toISOString(),
        format: 'strict_date_optional_time',
      },
    },
  };
}

/**
 * Combines the query bar, the filter pills and the time range into one
 * OpenSearch query, the way OpenSearch Dashboards does: DQL and pills filter
 * without scoring, Lucene scores, negated pills go to `must_not`, disabled
 * pills are left out.
 *
 * The time range has to sit directly in the root `bool.filter`: the server
 * refuses session and facet requests whose range it cannot find there.
 */
export function buildQuery(
  parts: QueryParts,
  range: ResolvedRange | undefined,
  fields: FieldDef[],
): BuiltQuery {
  const must: Dsl[] = [];
  const filter: Dsl[] = [];
  const mustNot: Dsl[] = [];

  const text = parts.query.trim();
  if (text !== '') {
    if (parts.language === 'lucene') {
      must.push(luceneToDsl(text));
    } else {
      try {
        filter.push(dqlToDsl(text, fields));
      } catch (error) {
        if (error instanceof DQLSyntaxError) {
          return { ok: false, error: error.shortMessage };
        }
        return { ok: false, error: error instanceof Error ? error.message : String(error) };
      }
    }
  }

  for (const pill of parts.filters) {
    if (pill.disabled) continue;
    (pill.negate ? mustNot : filter).push(filterToQuery(pill));
  }

  if (range) {
    filter.push(timeFilter(range));
  }

  return { ok: true, query: { bool: { must, filter, should: [], must_not: mustNot } } };
}
