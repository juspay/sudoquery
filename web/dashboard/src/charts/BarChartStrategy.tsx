import React from 'react';
import { ResponsiveBar } from '@nivo/bar';
import type { IChartStrategy, DataRow, CompatibilityResult, SingleChartConfig, ValidationResult } from '../types/chart';
import { getTextColor, getMutedTextColor, getAxisColor, getTooltipBackground, getTooltipTextColor } from '../utils/chartTheme';

export class BarChartStrategy implements IChartStrategy {
  id = 'bar-chart';
  name = 'Bar Chart';
  description = 'Best for comparing categories or time series.';

  // Format number with thousands separator
  private formatNumber(value: number): string {
    return value.toLocaleString();
  }

  validateConfig(data: DataRow[], explicitConfig?: SingleChartConfig): ValidationResult {
    const columns = data.length > 0 ? Object.keys(data[0]) : [];
    const missingFields: string[] = [];
    const invalidColumns: string[] = [];

    const xAxis = explicitConfig?.xAxis;
    const yAxis = explicitConfig?.yAxis;

    if (!xAxis) missingFields.push('xAxis');
    else if (!columns.includes(xAxis)) invalidColumns.push('xAxis');

    if (!yAxis) missingFields.push('yAxis');
    else {
      const yAxisCols = Array.isArray(yAxis) ? yAxis : [yAxis];
      yAxisCols.forEach(col => {
        if (!columns.includes(col)) invalidColumns.push('yAxis');
      });
    }

    return { valid: missingFields.length === 0 && invalidColumns.length === 0, missingFields, invalidColumns };
  }

  checkCompatibility(data: DataRow[]): CompatibilityResult {
    if (!data || data.length === 0) {
      return { isCompatible: false, confidence: 0 };
    }

    const sample = data[0];
    const keys = Object.keys(sample);

    // Find 1 dimension (string or number) and 1 number (Metric)
    const dimensionKey = keys.find((k) => typeof sample[k] === 'string');
    const numberKeys = keys.filter((k) => typeof sample[k] === 'number');

    if (dimensionKey && numberKeys.length >= 1) {
      const numberKey = numberKeys.find(k => k !== dimensionKey) || numberKeys[0];
      const isDate = /date|time|day|month/i.test(dimensionKey);

      return {
        isCompatible: true,
        confidence: isDate ? 0.85 : 0.8,
        matchedKeys: { x: dimensionKey, y: numberKey },
      };
    }

    // Fallback: use first number as dimension, second as value
    // For bar charts, prefer NON-time-like columns for x-axis (categories)
    // Time/date columns are better as y-axis (values like duration, count)
    if (numberKeys.length >= 2) {
      const timeLikeKey = numberKeys.find(k => /hour|day|time|date|month|year/i.test(k));
      const xKey = numberKeys.find(k => k !== timeLikeKey) || numberKeys[0];
      const yKey = timeLikeKey || numberKeys[1];

      return {
        isCompatible: true,
        confidence: 0.7,
        matchedKeys: { x: xKey, y: yKey },
      };
    }

    return { isCompatible: false, confidence: 0 };
  }

  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: import('../types/chart').SingleChartConfig): React.ReactNode {
    // Use explicit config if provided, otherwise fall back to auto-detected matchedKeys
    const x = explicitConfig?.xAxis || config.matchedKeys?.x;
    const y = explicitConfig?.yAxis || config.matchedKeys?.y;

    if (!x || !y) {
      return (
        <div style={{ color: '#595959', textAlign: 'center', padding: '20px' }}>
          Cannot render bar chart: missing axis configuration.
        </div>
      );
    }

    // Check if x-axis data is numeric
    const firstValue = data[0]?.[x];
    const isNumericX = typeof firstValue === 'number';

    // Check if y-axis data is numeric
    const firstYValue = data[0]?.[y];
    const isNumericY = typeof firstYValue === 'number';

    const yValues = data.map(row => row[y]);

    // For horizontal bar charts, reverse the data so first item appears at top
    const chartData = [...data].reverse();

    // Calculate left margin based on x-axis labels (left side for horizontal bars)
    // Use canvas for more accurate text width measurement
    const getTextWidth = (text: string, font = '14px DM Sans'): number => {
      if (typeof document === 'undefined') return text.length * 8;
      const canvas = document.createElement('canvas');
      const context = canvas.getContext('2d');
      if (!context) return text.length * 8;
      context.font = font;
      return context.measureText(text).width;
    };

    // Get all x-axis labels (these appear on the left for horizontal bars)
    const xLabels = data.map(row => String(row[x]));
    const maxLabelWidth = Math.max(...xLabels.map(label => getTextWidth(label)));

    // Add padding for the label + tick + some buffer
    // Also account for the value label at the end of bars
    const maxValueWidth = isNumericY
      ? getTextWidth(Math.max(...yValues.map(v => Number(v))).toLocaleString())
      : 0;

    const leftMargin = Math.max(100, maxLabelWidth + 20); // Minimum 100px, or label width + padding
    const rightMargin = Math.max(30, maxValueWidth + 20); // Space for value labels at bar ends

    return (
      <div style={{ height: yValues.length*30, minHeight: 200 }}>
        <ResponsiveBar
          data={chartData as Array<Record<string, string | number>>}
          keys={[y]}
          indexBy={x}
          layout="horizontal"
          defaultHeight={10}
          margin={{ top: 10, right: rightMargin, left: leftMargin, bottom: 40 }}
          padding={0.3}
          valueScale={{ type: 'linear' }}
          indexScale={{ type: 'band', round: true }}
          colors={['#1976d2']}
          borderRadius={4}
          borderWidth={1}
          borderColor={{ from: 'color', modifiers: [['darker', 0.3]] }}
          axisTop={null}
          axisRight={null}
          axisBottom={null}
          axisLeft={{
            legendPosition: 'middle',
            legendOffset: -80,
          }}
          labelPosition='end'
          labelOffset={2}
          labelTextColor={{ from: 'color', modifiers: [['darker', 1.6]] }}
          label={(d) => isNumericY ? this.formatNumber(Number(d.data[y])) : String(d.data[y])}
          tooltip={({ id, value, color }) => (
            <div
              style={{
                padding: '12px',
                background: getTooltipBackground(),
                color: getTooltipTextColor(),
                borderRadius: '4px',
                fontSize: '12px',
                boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
              }}
            >
              <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>{String(id)}</div>
              <div>Value: {isNumericY ? this.formatNumber(Number(value)) : String(value)}</div>
            </div>
          )}
          theme={{
            axis: {
              ticks: {
                text: {
                  fill: getMutedTextColor(),
                  fontFamily: "'DM Sans', sans-serif",
                  fontSize: 14,
                },
              },
              legend: {
                text: {
                  fill: getTextColor(),
                  fontFamily: "'DM Sans', sans-serif",
                  fontSize: 14,
                },
              },
            },
            grid: {
              line: {
                stroke: getAxisColor(),
                strokeDasharray: '4 4',
              },
            },
            tooltip: {
              container: {
                background: getTooltipBackground(),
                color: getTooltipTextColor(),
                fontFamily: "'JetBrains Mono', monospace",
                fontSize: 12,
              },
            },
          }}
          role="application"
          ariaLabel="Bar chart"
        />
      </div>
    );
  }
}
