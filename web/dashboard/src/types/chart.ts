import type { ReactNode } from 'react';

export type DataRow = Record<string, unknown>;

export interface CompatibilityResult {
  isCompatible: boolean;
  confidence: number;
  matchedKeys?: { [key: string]: string };
}

export interface ValidationResult {
  valid: boolean;
  missingFields: string[];
  invalidColumns: string[];
}

export interface IChartStrategy {
  id: string;
  name: string;
  description: string;
  checkCompatibility(data: DataRow[]): CompatibilityResult;
  validateConfig(data: DataRow[], explicitConfig?: SingleChartConfig): ValidationResult;
  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: SingleChartConfig): ReactNode;
}

/**
 * Configuration for a single chart type with explicit axis mappings.
 */
export interface SingleChartConfig {
  /** Unique identifier for this specific chart configuration (auto-generated if not provided) */
  id?: string;
  /** Chart type identifier (e.g., 'bar-chart', 'line-chart', 'pie-chart', 'sankey-chart', 'funnel-chart') */
  chartType?: string;
  /** Column name for x-axis (for bar/line charts) */
  xAxis?: string;
  /** Column name(s) for y-axis (for bar/line charts) */
  yAxis?: string | string[];
  /** Column name for labels (for funnel/pie charts) */
  labelAxis?: string;
  /** Column name for values (for funnel/sankey/pie charts) */
  valueAxis?: string;
}

/**
 * Chart configuration as an array of chart configurations.
 * Each item specifies a chart type with its axis mappings.
 */
export type ChartConfig = SingleChartConfig[];

/**
 * Wrapper for chart config that LLM returns
 */
export interface ChartConfigWrapper {
  charts: SingleChartConfig[];
}