import React from 'react';
import { Box, Table, TableBody, TableCell, TableHead, TableRow, Tooltip, useTheme } from "@mui/material";
import type { IChartStrategy, DataRow, CompatibilityResult, SingleChartConfig, ValidationResult } from '../types/chart';
import { getTextColor } from '../utils/chartTheme';

function aggregate(values: any[]): { display: string; raw: number } {
  const nums = values.map(Number).filter(v => !isNaN(v));
  if (nums.length > 0 && nums.length === values.length) {
    const sum = nums.reduce((a, b) => a + b, 0);
    return { display: formatNumber(sum), raw: sum };
  }
  return { display: String(values.length), raw: values.length };
}

function formatNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return value.toLocaleString();
}



export class SegmentedMatrixStrategy implements IChartStrategy {
  id = 'segmented-matrix';
  name = 'Segmented Matrix';
  description = 'Cross-tabulation matrix showing aggregated values across two dimensions.';

  validateConfig(data: DataRow[], explicitConfig?: SingleChartConfig): ValidationResult {
    const columns = data.length > 0 ? Object.keys(data[0]) : [];
    const missingFields: string[] = [];
    const invalidColumns: string[] = [];

    const xAxis = explicitConfig?.xAxis;
    const rawYAxis = explicitConfig?.yAxis;
    const yAxis = Array.isArray(rawYAxis) ? rawYAxis[0] : rawYAxis;
    const valueAxis = explicitConfig?.valueAxis;

    if (!xAxis) missingFields.push('xAxis');
    else if (!columns.includes(xAxis)) invalidColumns.push('xAxis');

    if (!yAxis) missingFields.push('yAxis');
    else if (!columns.includes(yAxis)) invalidColumns.push('yAxis');

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
    const stringKeys = keys.filter(k => typeof sample[k] === 'string');
    const numberKeys = keys.filter(k => typeof sample[k] === 'number');

    if (stringKeys.length >= 2) {
      return {
        isCompatible: true,
        confidence: 0.7,
        matchedKeys: {
          xAxis: stringKeys[0],
          yAxis: stringKeys[1],
          valueAxis: numberKeys.length > 0 ? numberKeys[0] : stringKeys[0],
        },
      };
    }

    return { isCompatible: false, confidence: 0 };
  }

  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: SingleChartConfig): React.ReactNode {
    const xAxis = explicitConfig?.xAxis || config.matchedKeys?.xAxis;
    const rawYAxis = explicitConfig?.yAxis || config.matchedKeys?.yAxis;
    const yAxis = Array.isArray(rawYAxis) ? rawYAxis[0] : rawYAxis;
    const valueAxis = explicitConfig?.valueAxis || config.matchedKeys?.valueAxis;

    if (!xAxis || !yAxis || !valueAxis) {
      return (
        <div style={{ color: '#595959', textAlign: 'center', padding: '20px' }}>
          Cannot render segmented matrix: missing axis configuration.
        </div>
      );
    }

    return <SegmentedMatrixInner xAxis={xAxis} yAxis={yAxis} valueAxis={valueAxis} data={data} />;
  }
}

function SegmentedMatrixInner({ xAxis, yAxis, valueAxis, data }: { xAxis: string; yAxis: string; valueAxis: string; data: any[] }) {
  const theme = useTheme();
  const isDark = theme.palette.mode === 'dark';

  const matrix: Record<string, Record<string, any[]>> = {};
  const yValuesSet = new Set<string>();

  data.forEach(row => {
    const xBucket = String(row[xAxis]);
    const yBucket = String(row[yAxis]);

    if (!matrix[xBucket]) {
      matrix[xBucket] = {};
    }
    if (!matrix[xBucket][yBucket]) {
      matrix[xBucket][yBucket] = [];
    }
    matrix[xBucket][yBucket].push(row[valueAxis]);
    yValuesSet.add(yBucket);
  });

  const xValues = Object.keys(matrix);
  const yValues = Array.from(yValuesSet);

  const separatorColor = '#000000';

  const cellColors = [
    { bg: '#264653', text: '#ffffff' },
    { bg: '#2a9d8f', text: '#ffffff' },
    { bg: '#e9c46a', text: '#264653' },
    { bg: '#f4a261', text: '#264653' },
    { bg: '#e76f51', text: '#ffffff' },
    { bg: '#a8dadc', text: '#264653' },
    { bg: '#457b9d', text: '#ffffff' },
    { bg: '#1d3557', text: '#ffffff' },
    { bg: '#e63946', text: '#ffffff' },
    { bg: '#f1faee', text: '#264653' },
  ];

  const getColor = (cellIndex: number) => cellColors[cellIndex % cellColors.length];

  const cellWrapperStyle = (isLastCol: boolean, isLastRow: boolean): React.CSSProperties => ({
    border: 'none',
    borderRight: isLastCol ? 'none' : `1px solid ${separatorColor}`,
    borderBottom: isLastRow ? 'none' : `1px solid ${separatorColor}`,
    padding: '5px',
    minWidth: 100,
    height: 100,
  });

  const getCellStyle = (rowIndex: number, colIndex: number): React.CSSProperties => {
    const color = getColor(rowIndex * yValues.length + colIndex);
    return {
      backgroundColor: color.bg,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      textAlign: 'center',
      fontWeight: 600,
      color: color.text,
      fontSize: 13,
      fontFamily: "'DM Sans', sans-serif",
      cursor: 'default',
      whiteSpace: 'nowrap',
      width: '100%',
      height: '100%',
    };
  };

  const headerCellStyle: React.CSSProperties = {
    fontWeight: 700,
    color: getTextColor(),
    backgroundColor: isDark ? '#1e1e1e' : '#f5f5f5',
    textAlign: 'center',
    border: 'none',
    padding: '6px 10px',
    fontSize: 13,
    fontFamily: "'DM Sans', sans-serif",
    whiteSpace: 'nowrap',
    verticalAlign: 'middle',
  };

  const cornerCellStyle: React.CSSProperties = {
    border: 'none',
    backgroundColor: isDark ? '#1e1e1e' : '#f5f5f5',
    padding: '6px 10px',
    whiteSpace: 'nowrap',
  };

  const rowHeaderCellStyle: React.CSSProperties = {
    ...headerCellStyle,
    textAlign: 'center',
    writingMode: 'vertical-lr',
    rotate: '180deg',
    fontWeight: 600,
    color: getTextColor(),
  };

  return (
    <Box sx={{ overflow: 'auto', borderRadius: 1 }}>
      <Table size="small" sx={{ width: 'fit-content', borderCollapse: 'collapse' }}>
        <TableHead>
          <TableRow>
            <TableCell sx={cornerCellStyle} />
            {yValues.map(y => (
              <TableCell key={y} sx={headerCellStyle}>{y}</TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {xValues.map((x, ri) => (
            <TableRow key={x} sx={{ height: 36 }}>
              <TableCell sx={rowHeaderCellStyle}>{x}</TableCell>
              {yValues.map((y, ci) => {
                const values = matrix[x]?.[y] ?? [];
                const agg = aggregate(values);
                const isLastCol = ci === yValues.length - 1;
                const isLastRow = ri === xValues.length - 1;
                return (
                  <TableCell key={y} sx={cellWrapperStyle(isLastCol, isLastRow)}>
                    <Box sx={getCellStyle(ri, ci)}>
                      <Tooltip
                        title={`${x} / ${y}: ${agg.display} (${values.length} item${values.length !== 1 ? 's' : ''})`}
                        arrow
                      >
                        <span>{agg.display}</span>
                      </Tooltip>
                    </Box>
                  </TableCell>
                );
              })}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  );
}
