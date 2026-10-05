import type { FilterPill, FilterValue } from './types';

type Dsl = Record<string, unknown>;

/** The un-negated OpenSearch clause for a pill; negation is applied by the caller. */
export function filterToQuery(pill: FilterPill): Dsl {
  switch (pill.operator) {
    case 'is':
      return { match_phrase: { [pill.field]: pill.value } };
    case 'is_one_of':
      return {
        bool: {
          should: (pill.values ?? []).map((value) => ({ match_phrase: { [pill.field]: value } })),
          minimum_should_match: 1,
        },
      };
    case 'exists':
      return { exists: { field: pill.field } };
    case 'range': {
      const bounds: Dsl = {};
      if (pill.gte !== undefined) bounds.gte = pill.gte;
      if (pill.lt !== undefined) bounds.lt = pill.lt;
      return { range: { [pill.field]: bounds } };
    }
  }
}

export function filterLabel(pill: FilterPill): string {
  switch (pill.operator) {
    case 'is':
      return `${pill.field}: ${String(pill.value)}`;
    case 'is_one_of':
      return `${pill.field}: ${(pill.values ?? []).map(String).join(', ')}`;
    case 'exists':
      return `${pill.field}: exists`;
    case 'range':
      return `${pill.field}: ${pill.gte ?? '*'} to ${pill.lt ?? '*'}`;
  }
}

function isFilter(pill: FilterPill, field: string, value: FilterValue): boolean {
  return pill.operator === 'is' && pill.field === field && pill.value === value;
}

/**
 * Adds a "field is value" pill, or "is not" when `negate`. A pill already
 * there for the same field and value is replaced, so clicking + after −
 * flips it instead of adding a contradiction.
 */
export function addValueFilter(
  filters: FilterPill[],
  field: string,
  value: FilterValue,
  negate: boolean,
): FilterPill[] {
  const pill: FilterPill = { field, operator: 'is', value, negate, disabled: false };
  const index = filters.findIndex((existing) => isFilter(existing, field, value));
  if (index === -1) return [...filters, pill];
  return filters.map((existing, position) => (position === index ? pill : existing));
}

export function addExistsFilter(filters: FilterPill[], field: string, negate: boolean): FilterPill[] {
  const pill: FilterPill = { field, operator: 'exists', negate, disabled: false };
  const index = filters.findIndex(
    (existing) => existing.operator === 'exists' && existing.field === field,
  );
  if (index === -1) return [...filters, pill];
  return filters.map((existing, position) => (position === index ? pill : existing));
}

/** Whether a field value can be put in a pill as it is. */
export function isFilterValue(value: unknown): value is FilterValue {
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';
}
