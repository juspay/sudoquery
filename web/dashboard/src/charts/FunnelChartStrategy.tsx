import React from 'react';
import { ResponsiveFunnel } from '@nivo/funnel';
import type { IChartStrategy, DataRow, CompatibilityResult, SingleChartConfig, ValidationResult } from '../types/chart';
import { getTextColor, getTooltipBackground, getTooltipTextColor } from '../utils/chartTheme';

export class FunnelChartStrategy implements IChartStrategy {
  id = 'funnel-chart';
  name = 'Funnel Chart';
  description = 'Best for conversion rates and sequential process stages.';

  // Format number with thousands separator
  private formatNumber(value: number | undefined): string {
    if (value === undefined || value === null) return '0';
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

    const stringKeys = keys.filter((k) => typeof sample[k] === 'string');
    const numberKey = keys.find((k) => typeof sample[k] === 'number');

    // Needs at least 1 string (for id/label) and 1 number (for value)
    if (stringKeys.length >= 1 && numberKey) {
      // Intelligent guessing based on column names
      const stringKey =
        stringKeys.find((k) =>
          /stage|step|phase|level|name|label|category/i.test(k),
        ) || stringKeys[0];

      return {
        isCompatible: true,
        confidence: 0.75,
        matchedKeys: { id: stringKey, value: numberKey, label: stringKey },
      };
    }

    return { isCompatible: false, confidence: 0 };
  }

  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: import('../types/chart').SingleChartConfig): React.ReactNode {
    // Use explicit config if provided, otherwise fall back to auto-detected matchedKeys
    const id = explicitConfig?.xAxis || config.matchedKeys?.id;
    const value = explicitConfig?.valueAxis || config.matchedKeys?.value;
    const label = explicitConfig?.labelAxis || config.matchedKeys?.label;

    if (!id || !value || !label) {
      return (
        <div style={{ color: '#666', textAlign: 'center', padding: '20px' }}>
          Cannot render funnel chart: missing axis configuration.
        </div>
      );
    }

    // Transform data to Nivo Funnel format
    const funnelData = data.map((row) => ({
      id: String(row[id]),
      value: Number(row[value]),
      label: String(row[label]),
    }));

    return (
      <div style={{ height: '100%', minHeight: 200 }}>
        <ResponsiveFunnel
          data={funnelData}
          margin={{ top: 20, right: 20, bottom: 20, left: 20 }}
          interpolation="smooth"
          direction="horizontal"
          valueFormat=">-.4s"
          colors={['#1976d2', '#42a5f5', '#64b5f6', '#90caf9', '#bbdefb']}
          borderWidth={1}
          borderColor={{ from: 'color', modifiers: [['darker', 0.3]] }}
          enableLabel={true}
          labelColor={getTextColor()}
          enableBeforeSeparators={true}
          beforeSeparatorLength={10}
          beforeSeparatorOffset={10}
          enableAfterSeparators={true}
          afterSeparatorLength={10}
          afterSeparatorOffset={10}
          labelFormat={(value) => this.formatNumber(value)}
          tooltip={({ part }) => (
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
              <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>{part.data.label}</div>
              <div>{value}: {this.formatNumber(part.data.value)}</div>
            </div>
          )}
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
        />
      </div>
    );
  }
}
