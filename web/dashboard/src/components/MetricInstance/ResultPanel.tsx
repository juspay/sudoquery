import { useState, useMemo, useEffect, useRef } from 'react';
import {
  Box,
  Typography,
  Paper,
  Stack,
  ToggleButton,
  ToggleButtonGroup,
  Alert,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  IconButton,
  Menu,
  MenuItem,
  Tooltip,
  useTheme,
} from '@mui/material';
import {
  Table as TableIcon,
  BarChartHorizontal as BarChartIcon,
  TrendingUp as LineChartIcon,
  PieChart as PieChartIcon,
  GitFork as SankeyIcon,
  Filter as FunnelIcon,
  Grid2x2 as MatrixIcon,
  Download as DownloadIcon,
} from 'lucide-react';
import type { QueryResponse, QueryStatistics } from '../../types/api';
import type { ChartConfig, SingleChartConfig } from '../../types/chart';
import { getCompatibleCharts, getStrategyById } from '../../charts/ChartRegistry';
import { downloadAsCSV, downloadAsJSON } from '../../utils/download';
import { ExpandableCellValue } from '../shared/ExpandableCellValue';
import { isExpandableValue } from '../../utils/cellValue';

// Local interface for MetricInstance (previously in types/pane.ts)
interface MetricInstance {
  id: string;
  metricId: string | null;
  name: string;
  description?: string;
  query: string;
  chart: string;
  variables: Record<string, unknown>;
  functions: Record<string, string>;
  isDirty: boolean;
  response: unknown;
  isRunning: boolean;
  showRaw: boolean;
  selectedChartId: string;
  panelOrientation: 'horizontal' | 'vertical';
  error: string | null;
  chartConfig?: ChartConfig;
}

// Chart type identifiers
const CHART_TYPES = ['bar-chart', 'line-chart', 'pie-chart', 'funnel-chart', 'sankey-chart', 'segmented-matrix'] as const;
type ChartType = typeof CHART_TYPES[number];

// Helper to get configured chart types from chartConfig
const getConfiguredChartTypes = (chartConfig: ChartConfig | undefined): ChartType[] => {
  if (!chartConfig) return [];
  return CHART_TYPES.filter((type) => chartConfig[type]);
};

const PRIMARY_COLOR = '#137fec';

// Format number with thousands separator
const formatNumber = (value: unknown): string => {
  if (typeof value === 'number') {
    return value.toLocaleString();
  }
  const numValue = Number(value);
  if (!isNaN(numValue)) {
    return numValue.toLocaleString();
  }
  return String(value ?? '');
};

// Map chart IDs to their icons and labels
const CHART_ICONS: Record<string, { icon: React.ElementType; label: string }> = {
  'bar-chart': { icon: BarChartIcon, label: 'Bar Chart' },
  'line-chart': { icon: LineChartIcon, label: 'Line Chart' },
  'pie-chart': { icon: PieChartIcon, label: 'Pie Chart' },
  'sankey-chart': { icon: SankeyIcon, label: 'Sankey Chart' },
  'funnel-chart': { icon: FunnelIcon, label: 'Funnel Chart' },
  'segmented-matrix': { icon: MatrixIcon, label: 'Matrix' },
};

const TABLE_OPTION = { id: 'table', icon: TableIcon, label: 'Table' };

interface ResultPanelProps {
  instance: MetricInstance;
  onUpdate: (updates: Partial<MetricInstance>) => void;
}

export function ResultPanel({ instance, onUpdate }: ResultPanelProps) {
  const theme = useTheme();
  const isDark = theme.palette.mode === 'dark';
  const [downloadMenuAnchor, setDownloadMenuAnchor] = useState<null | HTMLElement>(null);
  const userSelectedRef = useRef(false);

  const responseData = instance.response as QueryResponse | null;
  const hasResponse = !!instance.response;
  const hasError = !!responseData?.error;

  // Compute compatible charts from response data
  const compatibleCharts = useMemo(() => {
    if (responseData?.response?.data) {
      const compatible = getCompatibleCharts(responseData.response.data);
      return compatible.map((c) => ({ id: c.strategy.id, name: c.strategy.name }));
    }
    return [];
  }, [responseData]);

  // Get just the chart IDs for easier filtering
  const availableChartIds = useMemo(
    () => compatibleCharts.map(c => c.id),
    [compatibleCharts]
  );

  // Determine initial chart ID based on chartConfig if provided
  const initialChartId = useMemo(() => {
    const configuredTypes = getConfiguredChartTypes(instance.chartConfig);
    if (configuredTypes.length > 0) {
      // Return first configured chart type that is compatible
      const compatible = configuredTypes.find((type) => availableChartIds.includes(type));
      return compatible || null;
    }
    return null;
  }, [instance.chartConfig, availableChartIds]);

  // Auto-select chart ID based on available options
  useEffect(() => {
    if (!hasResponse) return;

    const currentSelection = instance.selectedChartId;

    // If current selection is not available, fallback to appropriate default
    if (currentSelection && currentSelection !== 'table') {
      // Check if it's a compatible chart type
      const isCompatibleType = availableChartIds.includes(currentSelection);

      if (!isCompatibleType) {
        // If chartConfig is provided, use first compatible chart
        if (initialChartId) {
          onUpdate({ selectedChartId: initialChartId });
        } else {
          onUpdate({ selectedChartId: TABLE_OPTION.id });
        }
        return;
      }
    }

    // If nothing selected, auto-select based on available options
    if (!currentSelection) {
      // If chartConfig is provided and its first chart is compatible, use it
      if (initialChartId) {
        onUpdate({ selectedChartId: initialChartId });
      }
      // If only one chart type is available, select it
      else if (availableChartIds.length === 1) {
        onUpdate({ selectedChartId: availableChartIds[0] });
      } else {
        // Otherwise default to table
        onUpdate({ selectedChartId: TABLE_OPTION.id });
      }
    }
  }, [instance.selectedChartId, hasResponse, compatibleCharts, onUpdate, initialChartId, instance.chartConfig, availableChartIds]);

  // Build all available view options: table + compatible charts
  // When chartConfig is provided: configured charts + table (at the end)
  // When no chartConfig: table + all compatible charts
  const availableViews = useMemo(() => {
    const tableOption = { id: TABLE_OPTION.id, icon: TABLE_OPTION.icon, label: TABLE_OPTION.label };

    const configuredTypes = getConfiguredChartTypes(instance.chartConfig);
    if (configuredTypes.length > 0) {
      // Show configured charts that are compatible, then table at the end
      const configuredViews = configuredTypes
        .filter(chartType => availableChartIds.includes(chartType))
        .map((chartType) => {
          return {
            id: chartType,
            icon: CHART_ICONS[chartType]?.icon || BarChartIcon,
            label: CHART_ICONS[chartType]?.label || chartType,
          };
        });
      return [...configuredViews, tableOption];
    } else {
      // No chartConfig, show table first, then all compatible charts
      return [
        tableOption,
        ...compatibleCharts.map((chart) => ({
          id: chart.id,
          icon: CHART_ICONS[chart.id]?.icon || BarChartIcon,
          label: chart.name,
        })),
      ];
    }
  }, [compatibleCharts, instance.chartConfig, availableChartIds]);

  const handleViewChange = (viewId: string) => {
    userSelectedRef.current = true;
    onUpdate({ selectedChartId: viewId });
  };

  const handleDownloadMenuClick = (event: React.MouseEvent<HTMLElement>) => {
    setDownloadMenuAnchor(event.currentTarget);
  };

  const handleDownloadMenuClose = () => {
    setDownloadMenuAnchor(null);
  };

  const handleDownloadCSV = () => {
    if (!data || data.length === 0) return;
    const timestamp = Date.now();
    downloadAsCSV(data, `metric_result_${timestamp}`);
    handleDownloadMenuClose();
  };

  const handleDownloadJSON = () => {
    if (!data || data.length === 0) return;
    const timestamp = Date.now();
    downloadAsJSON(data, `metric_result_${timestamp}`);
    handleDownloadMenuClose();
  };

  const statistics = responseData?.response?.statistics as QueryStatistics | undefined;
  const data = responseData?.response?.data as Record<string, unknown>[] | undefined;

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* Header */}
      <Box
        sx={{
          px: 2,
          py: 1,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          flexWrap: 'wrap',
          gap: 1,
        }}
      >
        <Stack direction="row" spacing={2} alignItems="center">
          <Stack>
            <Typography variant="subtitle2" fontWeight={500} sx={{ color: isDark ? 'text.primary' : '#625B71' }}>
              Results
              {hasResponse && statistics && (
                <span style={{ color: '#059669', marginLeft: 8 }}>
                  ({statistics.elapsed.toFixed(2)} sec, {formatNumber(statistics.rows_read)} rows, {formatNumber(statistics.bytes_read)} bytes)
                </span>
              )}
            </Typography>
            {instance.description && (
              <Typography variant="caption" color="text.secondary">
                {instance.description}
              </Typography>
            )}
          </Stack>
        </Stack>

        <Stack direction="row" spacing={0.5} alignItems="center" sx={{ bgcolor: theme.palette.background.paper, borderRadius: 1, p: 0.5, border: 1, borderColor: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)' }}>
          {/* View Options - Horizontal Icons */}
          <ToggleButtonGroup
            value={instance.selectedChartId || TABLE_OPTION.id}
            exclusive
            onChange={(_, value) => {
              if (value) handleViewChange(value);
            }}
            size="small"
            sx={{
              '& .MuiToggleButton-root': {
                borderRadius: 1,
                textTransform: 'none',
                fontWeight: 500,
                p: 0.5,
                border: 'none',
                '&.Mui-selected': {
                  bgcolor: PRIMARY_COLOR,
                  color: '#FFFFFF',
                  borderRadius: 1,
                },
                '&.Mui-disabled': {
                  opacity: 0.4,
                  color: '#9CA3AF',
                },
              },
            }}
          >
            {availableViews.map((view) => {
              const Icon = view.icon;
              return (
                <Tooltip key={view.id} title={view.label}>
                  <ToggleButton value={view.id} disabled={!hasResponse}>
                    <Icon size={16} />
                  </ToggleButton>
                </Tooltip>
              );
            })}
          </ToggleButtonGroup>
        </Stack>

        {/* Download Button */}
        <IconButton
          onClick={handleDownloadMenuClick}
          disabled={!hasResponse || !data || data.length === 0}
          size="small"
          title="Download"
          sx={{
            borderRadius: 9999,
            '&.Mui-disabled': {
              opacity: 0.4,
              color: '#9CA3AF',
            },
          }}
        >
          <DownloadIcon size={16} />
        </IconButton>

        {/* Download Menu */}
        <Menu
          anchorEl={downloadMenuAnchor}
          open={Boolean(downloadMenuAnchor)}
          onClose={handleDownloadMenuClose}
          anchorOrigin={{
            vertical: 'bottom',
            horizontal: 'right',
          }}
          transformOrigin={{
            vertical: 'top',
            horizontal: 'right',
          }}
        >
          <MenuItem onClick={handleDownloadCSV}>Download as CSV</MenuItem>
          <MenuItem onClick={handleDownloadJSON}>Download as JSON</MenuItem>
        </Menu>
      </Box>

      {/* Error Display */}
      {instance.error && (
        <Alert severity="error" onClose={() => onUpdate({ error: null })} sx={{ m: 1 }}>
          {instance.error}
        </Alert>
      )}

      {/* Content */}
      {hasError ? (
        <Box sx={{ flex: 1, overflow: 'auto', p: 1 }}>
          <Alert severity="error" sx={{ mb: 1 }}>
            <Typography variant="subtitle2" fontWeight={600} gutterBottom>
              Query Error
            </Typography>
            <Typography variant="body2" sx={{ fontFamily: 'monospace', whiteSpace: 'pre-wrap' }}>
              {responseData?.error}
            </Typography>
          </Alert>
        </Box>
      ) : !hasResponse ? (
        <Box
          sx={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            p: 4,
          }}
        >
          <Typography variant="body2" color="text.secondary">
            Run the metric to see results here
          </Typography>
        </Box>
      ) : (
        <Box sx={{ flex: 1, overflow: 'auto', p: 1, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          {/* Chart View */}
          {instance.selectedChartId && instance.selectedChartId !== 'table' && data && (
            <Paper sx={{ overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
              <Box sx={{ height: 400, position: 'relative' }}>
                {(() => {
                  const selectedChartType = instance.selectedChartId as ChartType;
                  const selectedChartConfig = instance.chartConfig?.[selectedChartType];

                  const strategy = getStrategyById(selectedChartType);
                  const compatible = getCompatibleCharts(data).find(
                    (c) => c.strategy.id === selectedChartType,
                  );

                  if (strategy && compatible) {
                    const validation = strategy.validateConfig(data, selectedChartConfig);
                    const effectiveConfig = validation.valid ? selectedChartConfig : undefined;
                    return strategy.render(data, compatible.config, effectiveConfig) as React.ReactNode;
                  }
                  // If user clicked this chart, show message; otherwise auto-switch to table
                  if (userSelectedRef.current) {
                    return (
                      <Alert severity="info">
                        This chart type is not compatible with the current data structure.
                      </Alert>
                    );
                  }
                  // Auto-switch to table on first render if incompatible
                  setTimeout(() => onUpdate({ selectedChartId: TABLE_OPTION.id }), 0);
                  return null;
                })()}
              </Box>
            </Paper>
          )}

          {/* Table View */}
          {instance.selectedChartId === 'table' && data && (
            <TableContainer component={Paper} sx={{ flex: 1, overflow: 'auto', minHeight: 0 }}>
              <Table stickyHeader size="small" sx={{ fontFamily: "'JetBrains Mono', monospace" }}>
                <TableHead>
                  <TableRow>
                    {data.length > 0 &&
                      Object.keys(data[0]).map((key) => (
                        <TableCell key={key} sx={{ fontFamily: "'JetBrains Mono', monospace", fontWeight: 600 }}>{key}</TableCell>
                      ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {data.map((row: Record<string, unknown>, idx: number) => (
                    <TableRow key={idx}>
                      {Object.values(row).map((val: unknown, cellIdx: number) => (
                        <TableCell key={cellIdx} sx={{ fontFamily: "'JetBrains Mono', monospace" }}>
                          {isExpandableValue(val) ? <ExpandableCellValue value={val} /> : formatNumber(val)}
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          )}
        </Box>
      )}
    </Box>
  );
}
