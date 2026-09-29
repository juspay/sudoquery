import React from 'react';
import { ResponsiveLine } from '@nivo/line';
import type { IChartStrategy, DataRow, CompatibilityResult, SingleChartConfig, ValidationResult } from '../types/chart';
import { getTextColor, getMutedTextColor, getAxisColor, getTooltipBackground, getTooltipTextColor } from '../utils/chartTheme';
import { prepare, layout, measureNaturalWidth, prepareWithSegments } from '@chenglou/pretext'

export class LineChartStrategy implements IChartStrategy {
  id = 'line-chart';
  name = 'Line Chart';
  description = 'Best for showing trends over time or continuous data.';

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
        confidence: isDate ? 0.9 : 0.85,
        matchedKeys: { x: dimensionKey, y: numberKey },
      };
    }

    // Fallback: use first number as dimension, second as value
    // Prefer columns with time/date-like names for x-axis
    if (numberKeys.length >= 2) {
      const timeLikeKey = numberKeys.find(k => /hour|day|time|date|month|year/i.test(k));
      const xKey = timeLikeKey || numberKeys[0];
      const yKey = numberKeys.find(k => k !== xKey) || numberKeys[1];

      return {
        isCompatible: true,
        confidence: 0.8,
        matchedKeys: { x: xKey, y: yKey },
      };
    }

    return { isCompatible: false, confidence: 0 };
  }

  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: import('../types/chart').SingleChartConfig): React.ReactNode {
    // Use explicit config if provided, otherwise fall back to auto-detected matchedKeys
    const x = explicitConfig?.xAxis || config.matchedKeys?.x;

    // Normalize yAxis to array
    const yAxes = Array.isArray(explicitConfig?.yAxis)
      ? explicitConfig.yAxis
      : [explicitConfig?.yAxis || config.matchedKeys?.y];

    if (!x || !yAxes[0]) {
      return (
        <div style={{ color: '#595959', textAlign: 'center', padding: '20px' }}>
          Cannot render line chart: missing axis configuration.
        </div>
      );
    }

    const isMultiSeries = yAxes.length > 1;

    // Check if x-axis data is numeric
    const firstValue = data[0]?.[x];
    const isNumericX = typeof firstValue === 'number';

    // Create series data with original values stored
    const seriesData = yAxes.map((yAxisColumn) => ({
      id: yAxisColumn,
      data: data.map((row) => ({
        x: isNumericX ? Number(row[x]) : String(row[x]),
        y: Number(row[yAxisColumn]),
        originalValue: Number(row[yAxisColumn]), // Store original value for tooltips
      })),
    }));

    // Sort each series by x when numeric
    const sortedSeriesData = isNumericX
      ? seriesData.map((series) => ({
          ...series,
          data: [...series.data].sort((a, b) => Number(a.x) - Number(b.x)),
        }))
      : seriesData;

    // For multi-series, normalize each series to 0-100% range
    const normalizedSeriesData = isMultiSeries
      ? sortedSeriesData.map((series) => {
          const yValues = series.data.map(d => d.y);
          const minY = Math.min(...yValues);
          const maxY = Math.max(...yValues);
          const range = maxY - minY;

          return {
            ...series,
            data: series.data.map((point) => ({
              ...point,
              y: range > 0 ? ((point.y - minY) / range) * 100 : 0, // Normalize to 0-100
              minValue: minY,
              maxValue: maxY,
            })),
          };
        })
      : sortedSeriesData;

    // Calculate tick values to prevent overlapping
    const maxTicks = 8;
    const getTickValues = (values: unknown[]) => {
      if (values.length <= maxTicks) return values;
      const step = Math.ceil(values.length / maxTicks);
      return values.filter((_, i) => i % step === 0);
    };

    const xValues = normalizedSeriesData[0]?.data.map(d => d.x) || [];

    // Use different colors for each series
    const colors = isMultiSeries
      ? ['#1976d2', '#388e3c', '#f57c00', '#7b1fa2', '#c2185b']
      : ['#1976d2'];

    // Calculate margins based on label widths using canvas
    const getTextWidth = (text: string, font = '14px DM Sans'): number => {
      if (typeof document === 'undefined') return text.length * 8;
      const canvas = document.createElement('canvas');
      const context = canvas.getContext('2d');
      if (!context) return text.length * 8;
      context.font = font;
      return context.measureText(text).width;
    };

    // Calculate left margin based on y-axis tick labels
    let maxYLabelWidth = 0;
    if (isMultiSeries) {
      maxYLabelWidth = getTextWidth('100%');
    } else {
      // Get max value from all series data
      const allValues = normalizedSeriesData.flatMap(s => s.data.map(d => d.y));
      const maxValue = Math.max(...allValues);
      const minValue = Math.min(...allValues);
      maxYLabelWidth = Math.max(
        getTextWidth(this.formatNumber(maxValue)),
        getTextWidth(this.formatNumber(minValue))
      );
    }

    // Calculate bottom margin based on x-axis labels
    const xLabels = isNumericX
      ? [String(Math.min(...xValues.map(Number))), String(Math.max(...xValues.map(Number)))]
      : xValues.slice(0, 10).map(String); // Sample first 10 for performance
    const maxXLabelWidth = Math.max(...xLabels.map(label => getTextWidth(label)));

    // Add space for legend if multi-series
    const rightMargin = isMultiSeries ? 120 : 30;

    // Add padding for axis labels + legend offset + tick marks
    const leftMargin = Math.max(60, maxYLabelWidth + 20);
    const bottomMargin = Math.max(60, Math.min(maxXLabelWidth + 20, 100)); // Cap at 100px

    // Y-axis configuration
    const yAxisConfig = isMultiSeries
      ? {
          // legend: 'Relative %',
          legendPosition: 'middle' as const,
          legendOffset: -Math.max(50, leftMargin - 10),
          format: (value: number) => `${Math.round(value)}%`,
          tickValues: [0, 25, 50, 75, 100],
        }
      : {
          // legend: yAxes[0],
          legendPosition: 'middle' as const,
          legendOffset: -Math.max(50, leftMargin - 10),
          format: (value: number) => this.formatNumber(Number(value)),
        };

    // get maximum x label
    function getMaxXLabelWidth() {
      const xLabels = isNumericX
        ? [String(Math.min(...xValues.map(Number))), String(Math.max(...xValues.map(Number)))]
        : xValues.slice(0, 10).map(String); // Sample first 10 for performance
      let maxXLabelWidth = 0;
      xLabels.forEach(label => {
        const prepared = prepareWithSegments(label, '14px DM Sans');
        const layoutResult = measureNaturalWidth(prepared);
        maxXLabelWidth = Math.max(maxXLabelWidth, layoutResult);
      });

      console.log('Max X Label Width:', maxXLabelWidth);
      return maxXLabelWidth;
    }

    return (
      // <div style={{ height: '100%', minHeight: 400, width: Math.max(getMaxXLabelWidth() * 1.1, 100) * xValues.length, maxWidth: '100%' }}>
      <div style={{ height: '100%', minHeight: 400, width: '100%', maxWidth: '100%' }}>
        <ResponsiveLine
          data={normalizedSeriesData}
          margin={{ top: 10, right: rightMargin, bottom: bottomMargin, left: leftMargin }}
          xScale={{ type: isNumericX ? 'linear' : 'point' }}
          yScale={{ type: 'linear', min: 0, max: isMultiSeries ? 100 : 'auto' }}
          axisBottom={{
            legendPosition: 'middle',
            legendOffset: 36,
            tickValues: isNumericX ? undefined : getTickValues(xValues),
            format: isNumericX ? (value) => String(Math.floor(Number(value))) : undefined,
          }}
          axisLeft={yAxisConfig}
          enableGridX={false}
          pointSize={10}
          pointBorderWidth={2}
          pointBorderColor={{ from: 'serieColor' }}
          pointLabel="y"
          pointLabelYOffset={-12}
          useMesh={true}
          colors={colors}
          curve="monotoneX"
          tooltip={({ point }) => (
            <div
              style={{
                background: getTooltipBackground(),
                color: getTooltipTextColor(),
                padding: '8px 12px',
                borderRadius: '4px',
                fontSize: '12px',
                boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
                zIndex: 9999,
                position: 'relative',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                <div
                  style={{
                    width: '10px',
                    height: '10px',
                    borderRadius: '50%',
                    backgroundColor: point.color,
                  }}
                />
                <div style={{ fontWeight: 'bold' }}>{point.serieId}</div>
              </div>
              <div>{x}: {isNumericX ? Math.floor(Number(point.data.x)) : point.data.x}</div>
              <div style={{ marginTop: '4px' }}>
                Value: {isMultiSeries
                  ? `${this.formatNumber(point.data.originalValue)}`
                  : this.formatNumber(point.data.y)}
              </div>
            </div>
          )}
          legends={isMultiSeries ? [{
            anchor: 'bottom-left',
            direction: 'column',
            justify: false,
            translateX: 0,
            translateY: 100,
            itemsSpacing: 0,
            itemWidth: 80,
            itemHeight: 20,
            itemOpacity: 0.75,
            symbolSize: 12,
            symbolShape: 'circle',
            effects: [{
              on: 'hover',
              style: {
                itemOpacity: 1,
              },
            }],
          }] : undefined}
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
        />
      </div>
    );
  }
}
