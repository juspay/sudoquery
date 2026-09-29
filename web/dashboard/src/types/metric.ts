type MetricSource = 'chat' | 'manual';

type DynamicQueryInputType = 'number' | 'string' | 'date' | 'datetime' | 'direction' | 'searchable_dropdown'

// Layout configuration as array of rows. Each inner array represents a horizontal row of input fields.
// Example: [['start_date', 'end_date'], ['event_name', 'device_type']] creates two rows with two items each.
type DynamicQueryLayout = string[][];

interface DynamicQueryInput {
  description: string,
  default: string,
  inputType: DynamicQueryInputType,
  // drop down query should always return single column
  // for searchable_dropdown, the query can use {variable_name} placeholder which will be replaced with %search_term%
  dropDownQuery: string | undefined
}

/*
 * query contains placeholders with syntax {variable_key}
 * to generate a query we have to replace {variable_key} with variables[variable_key]
* */
export interface DynamicQuery {
  query: string,
  variables: Record<string, DynamicQueryInput>,
  layout?: DynamicQueryLayout
}

export interface SavedMetric {
  id: string;
  label: string;
  description: string;
  query: string;
  source: MetricSource;
  createdAt: number;
  updatedAt: number;
  lastRunTime?: number;
  collectionPath?: string;
  dynamicQuery?: DynamicQuery;
  chartConfig?: import('./chart').ChartConfig;
}

