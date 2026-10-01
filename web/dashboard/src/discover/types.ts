/** An event as the API returns it: a canonical event without its tenant. */
export type EventDoc = Record<string, unknown>;

export type QueryLanguage = 'dql' | 'lucene';

export type SortOrder = 'asc' | 'desc';

/**
 * Both ends are either an absolute ISO timestamp or a token relative to the
 * moment the search runs: `now`, `now-15m`, `now-7d`.
 */
export interface TimeRange {
  from: string;
  to: string;
}

export interface ResolvedRange {
  from: Date;
  to: Date;
}

export type FilterValue = string | number | boolean;

export type FilterOperator = 'is' | 'is_one_of' | 'exists' | 'range';

/** A filter pill. `negate` turns it into "is not", "does not exist", … */
export interface FilterPill {
  field: string;
  operator: FilterOperator;
  /** `is` */
  value?: FilterValue;
  /** `is_one_of` */
  values?: FilterValue[];
  /** `range`: `gte` inclusive, `lt` exclusive */
  gte?: FilterValue;
  lt?: FilterValue;
  negate: boolean;
  disabled: boolean;
}

/** Everything that defines a search; kept in the page URL. */
export interface DiscoverState {
  query: string;
  language: QueryLanguage;
  time: TimeRange;
  filters: FilterPill[];
  /** Fields shown as table columns; empty shows the document summary. */
  columns: string[];
  order: SortOrder;
}

export type FieldType = 'string' | 'number' | 'boolean' | 'date' | 'ip' | 'object' | 'unknown';

export interface FieldDef {
  name: string;
  type: FieldType;
  /** The server can count this field's values over the whole result. */
  facetable: boolean;
}
