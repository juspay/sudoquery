/*
 * Copyright OpenSearch Contributors
 * SPDX-License-Identifier: Apache-2.0
 */

// Not an upstream file. These are the types the kuery code imports from
// outside its own directory in OpenSearch Dashboards, reduced to what kuery
// reads; see README.md.

// From src/plugins/opensearch_dashboards_utils/common/typed_json.ts
export type JsonValue = null | boolean | number | string | JsonObject | JsonArray;

export interface JsonObject {
  [key: string]: JsonValue;
}

export type JsonArray = JsonValue[];

// From src/plugins/data/common/index_patterns/fields/types.ts
export interface IFieldSubType {
  multi?: { parent: string };
  nested?: { path: string };
}

export interface IFieldType {
  name: string;
  /** Only `'date'` changes how a query is built. */
  type: string;
  script?: string;
  lang?: string;
  count?: number;
  esTypes?: string[];
  aggregatable?: boolean;
  filterable?: boolean;
  searchable?: boolean;
  sortable?: boolean;
  visualizable?: boolean;
  readFromDocValues?: boolean;
  /** Script queries are not generated for scripted fields; see README.md. */
  scripted?: boolean;
  subType?: IFieldSubType;
  displayName?: string;
}

// From src/plugins/data/common/index_patterns/types.ts. Upstream requires
// `title`; kuery never reads it.
export interface IIndexPattern {
  fields: IFieldType[];
  title?: string;
  id?: string;
}

// From src/plugins/data/common/opensearch_query/filters/range_filter.ts
export interface RangeFilterParams {
  from?: number | string;
  to?: number | string;
  gt?: number | string;
  lt?: number | string;
  gte?: number | string;
  lte?: number | string;
  format?: string;
}
