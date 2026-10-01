import { DEFAULT_RANGE } from './timeRange';
import type { DiscoverState, FilterOperator, FilterPill, FilterValue } from './types';

export const DEFAULT_STATE: DiscoverState = {
  query: '',
  language: 'dql',
  time: DEFAULT_RANGE,
  filters: [],
  columns: [],
  order: 'desc',
};

const OPERATORS: FilterOperator[] = ['is', 'is_one_of', 'exists', 'range'];

function isValue(value: unknown): value is FilterValue {
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';
}

/** Keeps only well-formed pills, so a hand-edited URL cannot break the page. */
function parseFilters(raw: string | null): FilterPill[] {
  if (!raw) return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];

  const pills: FilterPill[] = [];
  for (const entry of parsed) {
    if (typeof entry !== 'object' || entry === null) continue;
    const { field, operator, value, values, gte, lt, negate, disabled } = entry as Record<
      string,
      unknown
    >;
    if (typeof field !== 'string' || field === '') continue;
    if (!OPERATORS.includes(operator as FilterOperator)) continue;

    const pill: FilterPill = {
      field,
      operator: operator as FilterOperator,
      negate: negate === true,
      disabled: disabled === true,
    };
    if (pill.operator === 'is') {
      if (!isValue(value)) continue;
      pill.value = value;
    } else if (pill.operator === 'is_one_of') {
      if (!Array.isArray(values) || values.length === 0 || !values.every(isValue)) continue;
      pill.values = values;
    } else if (pill.operator === 'range') {
      if (!isValue(gte) && !isValue(lt)) continue;
      if (isValue(gte)) pill.gte = gte;
      if (isValue(lt)) pill.lt = lt;
    }
    pills.push(pill);
  }
  return pills;
}

/** Drops the flags that are false, to keep the URL short. */
function serializeFilters(filters: FilterPill[]): string {
  return JSON.stringify(
    filters.map(({ negate, disabled, ...rest }) => ({
      ...rest,
      ...(negate ? { negate } : {}),
      ...(disabled ? { disabled } : {}),
    })),
  );
}

export function parseState(params: URLSearchParams): DiscoverState {
  const from = params.get('from');
  const to = params.get('to');
  const columns = params.get('cols');
  return {
    query: params.get('q') ?? DEFAULT_STATE.query,
    language: params.get('lang') === 'lucene' ? 'lucene' : 'dql',
    time: from && to ? { from, to } : DEFAULT_STATE.time,
    filters: parseFilters(params.get('f')),
    columns: columns ? columns.split(',').filter(Boolean) : [],
    order: params.get('sort') === 'asc' ? 'asc' : 'desc',
  };
}

/** Only what differs from the defaults is written. */
export function serializeState(state: DiscoverState): URLSearchParams {
  const params = new URLSearchParams();
  if (state.query !== DEFAULT_STATE.query) params.set('q', state.query);
  if (state.language !== DEFAULT_STATE.language) params.set('lang', state.language);
  if (state.time.from !== DEFAULT_STATE.time.from || state.time.to !== DEFAULT_STATE.time.to) {
    params.set('from', state.time.from);
    params.set('to', state.time.to);
  }
  if (state.filters.length > 0) params.set('f', serializeFilters(state.filters));
  if (state.columns.length > 0) params.set('cols', state.columns.join(','));
  if (state.order !== DEFAULT_STATE.order) params.set('sort', state.order);
  return params;
}
