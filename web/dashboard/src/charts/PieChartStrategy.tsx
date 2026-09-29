import React from 'react';
import { ResponsivePie } from '@nivo/pie';
import type { IChartStrategy, DataRow, CompatibilityResult, SingleChartConfig, ValidationResult } from '../types/chart';
import { getTextColor, getMutedTextColor, getTooltipBackground, getTooltipTextColor } from '../utils/chartTheme';

export class PieChartStrategy implements IChartStrategy {
  id = 'pie-chart';
  name = 'Pie Chart';
  description = 'Good for part-to-whole comparison (small datasets).';

  // Format number with thousands separator
  private formatNumber(value: number): string {
    return value.toLocaleString();
  }

  validateConfig(data: DataRow[], explicitConfig?: SingleChartConfig): ValidationResult {
    const columns = data.length > 0 ? Object.keys(data[0]) : [];
    const missingFields: string[] = [];
    const invalidColumns: string[] = [];

    const labelAxis = explicitConfig?.labelAxis;
    const valueAxis = explicitConfig?.valueAxis;

    if (!labelAxis) missingFields.push('labelAxis');
    else if (!columns.includes(labelAxis)) invalidColumns.push('labelAxis');

    if (!valueAxis) missingFields.push('valueAxis');
    else if (!columns.includes(valueAxis)) invalidColumns.push('valueAxis');

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

      return {
        isCompatible: true,
        confidence: 0.6,
        matchedKeys: { name: dimensionKey, value: numberKey },
      };
    }

    // Fallback: use first number as dimension, second as value
    if (numberKeys.length >= 2) {
      return {
        isCompatible: true,
        confidence: 0.5,
        matchedKeys: { name: numberKeys[0], value: numberKeys[1] },
      };
    }

    return { isCompatible: false, confidence: 0 };
  }

  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: import('../types/chart').SingleChartConfig): React.ReactNode {
    // Use explicit config if provided, otherwise fall back to auto-detected matchedKeys
    // Pie chart uses labelAxis/valueAxis, but also accept xAxis/yAxis as fallback
    const name = explicitConfig?.labelAxis || explicitConfig?.xAxis || config.matchedKeys?.name;
    const value = explicitConfig?.valueAxis || explicitConfig?.yAxis || config.matchedKeys?.value;

    if (!name || !value) {
      return (
        <div style={{ color: '#595959', textAlign: 'center', padding: '20px' }}>
          Cannot render pie chart: missing axis configuration.
        </div>
      );
    }

    const sortedData = [...data].sort((a, b) => Number(b[value]) - Number(a[value]));
    const topData = sortedData.slice(0, 10);
    const hasMore = sortedData.length > 10;

    const pieData = topData.map((row) => ({
      id: typeof row[name] === 'number' ? row[name] : String(row[name]),
      label: typeof row[name] === 'number' ? row[name] : String(row[name]),
      value: Number(row[value]),
    }));

    return (
      <div style={{ height: '100%', minHeight: hasMore ? 440 : 400, display: 'flex', flexDirection: 'column' }}>
        {hasMore && (
          <div style={{ textAlign: 'center', padding: '4px 0', fontSize: '12px', color: getMutedTextColor() }}>
            Showing top 10 of {sortedData.length} categories
          </div>
        )}
        <div style={{ flex: 1 }}>
        <ResponsivePie
          data={pieData}
          margin={{ top: 20, right: 20, bottom: 20, left: 20 }}
          innerRadius={0}
          padAngle={0.7}
          cornerRadius={3}
          activeOuterRadiusOffset={8}
          borderWidth={1}
          borderColor={{ from: 'color', modifiers: [['darker', 0.2]] }}
          colors={['#264653', '#2a9d8f', '#e9c46a', '#f4a261', '#e76f51', '#a8dadc', '#457b9d', '#1d3557', '#e63946', '#f1faee']}
          arcLinkLabelsSkipAngle={10}
          arcLinkLabelsTextColor={getTextColor()}
          arcLinkLabelsThickness={2}
          arcLinkLabelsColor={{ from: 'color' }}
          enableArcLabels={false}
          legends={[
            {
              anchor: 'bottom',
              direction: 'row',
              justify: false,
              translateX: 0,
              translateY: 56,
              itemsSpacing: 0,
              itemWidth: 100,
              itemHeight: 18,
              itemTextColor: getMutedTextColor(),
              itemDirection: 'left-to-right',
              itemOpacity: 1,
              symbolSize: 18,
              symbolShape: 'circle',
              effects: [
                {
                  on: 'hover',
                  style: {
                    itemTextColor: getTextColor(),
                  },
                },
              ],
            },
          ]}
          theme={{
            tooltip: {
              container: {
                background: getTooltipBackground(),
                color: getTooltipTextColor(),
                fontFamily: "'JetBrains Mono', monospace",
                fontSize: 12,
              },
            },
          }}
          tooltip={({ datum }) => (
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
              <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>{datum.label}</div>
              <div>Value: {this.formatNumber(datum.value)}</div>
            </div>
          )}
        />
        </div>
      </div>
    );
  }
}
