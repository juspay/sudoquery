import { useCallback, useEffect, useRef, useState } from 'react';
import { useTheme } from '@mui/material/styles';
import { Box, Paper, Typography, ToggleButton, ToggleButtonGroup, Tooltip, IconButton, Skeleton, Dialog, DialogTitle, DialogContent } from '@mui/material';
import type { ReactNode } from 'react';
import {
  MoreVertical,
  Terminal as TerminalIcon,
  Download as DownloadIcon,
  Bug as BugIcon,
  BarChartHorizontal as BarChartIcon,
  Table as TableIcon,
  TrendingUp as LineChartIcon,
  PieChart as PieChartIcon,
  GitBranch as SankeyIcon,
  Filter as FunnelIcon,
  Grid2x2 as MatrixIcon,
  X as CloseIcon,
  Copy as CopyIcon,
  FileSpreadsheet,
  FileJson,
} from 'lucide-react';
import { fontFamilyBody, colorBlue } from '../../theme/tokens';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark, oneLight } from 'react-syntax-highlighter/dist/esm/styles/prism';
import type { MetricData } from '../../types/chat';
import type { QueryResponse, QueryStatistics } from '../../types/api';
import type { ChartConfigWrapper, SingleChartConfig } from '../../types/chart';
import { executeQuery } from '../../services/clickhouseService';
import { getStrategyById, getCompatibleCharts } from '../../charts/ChartRegistry';
import { downloadAsCSV, downloadAsJSON } from '../../utils/download';
import { useDebugMode } from '../../hooks/useDebugMode';
import { ExpandableCellValue } from '../shared/ExpandableCellValue';
import { isExpandableValue } from '../../utils/cellValue';

interface InlineMetricProps {
  metricData: MetricData;
  headerEnd?: ReactNode;
  footer?: ReactNode;
  additionalActions?: ActionItem[];
}

const CHART_TYPES = ['bar-chart', 'line-chart', 'pie-chart', 'funnel-chart', 'sankey-chart', 'segmented-matrix'] as const;
type ChartType = typeof CHART_TYPES[number];

export interface ActionItem {
  id: string;
  label: string;
  icon: React.ElementType;
  onClick: () => void;
  disabled?: boolean;
}

function ActionsDropdown({
  actions,
}: {
  actions: ActionItem[];
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState<'down' | 'up'>('down');
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const calculatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const dropdownHeight = 200;
    const spaceBelow = window.innerHeight - rect.bottom;
    const spaceAbove = rect.top;
    if (spaceBelow < dropdownHeight && spaceAbove > dropdownHeight) {
      setPosition('up');
    } else {
      setPosition('down');
    }
  }, []);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    if (isOpen) {
      calculatePosition();
    }
  }, [isOpen, calculatePosition]);

  return (
    <Box sx={{ position: 'relative' }} ref={dropdownRef}>
      <Tooltip title="Actions">
        <IconButton
          ref={triggerRef}
          size="small"
          onClick={() => setIsOpen(!isOpen)}
          sx={{
            opacity: 0.7,
            '&:hover': { opacity: 1 },
          }}
        >
          <MoreVertical size={16} />
        </IconButton>
      </Tooltip>

      {isOpen && (
        <Box
          sx={{
            position: 'absolute',
            right: 0,
            minWidth: '180px',
            maxWidth: '280px',
            backgroundColor: '#ffffff',
            borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0, 0, 0, 0.15)',
            zIndex: 1000,
            overflow: 'hidden',
            ...(position === 'down'
              ? { top: 'calc(100% + 4px)' }
              : { bottom: 'calc(100% + 4px)' }
            ),
          }}
        >
          {actions.map((action, index) => {
            const Icon = action.icon;
            if (action.id === 'separator') {
              return (
                <Box
                  key={`separator-${index}`}
                  sx={{
                    height: '1px',
                    backgroundColor: '#e5e7eb',
                    margin: '4px 0',
                  }}
                />
              );
            }
            // Check if next item is a separator, if so don't add borderBottom
            const nextIsSeparator = index < actions.length - 1 && actions[index + 1]?.id === 'separator';
            const hasBorderBottom = index < actions.length - 1 && !nextIsSeparator;
            
            return (
              <Box
                key={action.id}
                onClick={() => {
                  if (!action.disabled) {
                    action.onClick();
                    setIsOpen(false);
                  }
                }}
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  padding: '10px 14px',
                  fontSize: '13px',
                  fontFamily: fontFamilyBody,
                  color: action.disabled ? '#9ca3af' : '#18160f',
                  cursor: action.disabled ? 'not-allowed' : 'pointer',
                  backgroundColor: 'transparent',
                  borderBottom: hasBorderBottom ? '1px solid #f3f4f6' : 'none',
                  '&:hover': {
                    backgroundColor: action.disabled ? 'transparent' : '#f3f4f6',
                  },
                }}
              >
                <Icon size={16} color={action.disabled ? '#9ca3af' : '#6b7280'} />
                <Box sx={{ fontWeight: 400 }}>{action.label}</Box>
              </Box>
            );
          })}
        </Box>
      )}
    </Box>
  );
}

const getConfiguredChartTypes = (chartConfig: ChartConfigWrapper | undefined): ChartType[] => {
  if (!chartConfig?.charts) return [];
  return chartConfig.charts
    .map(x => x.chartType as ChartType)
    .filter((type): type is ChartType => CHART_TYPES.includes(type));
};

const getChartConfig = (chartConfig: ChartConfigWrapper | undefined, chartType: ChartType): SingleChartConfig | undefined => {
  return chartConfig?.charts?.find(c => c.chartType === chartType);
};

const CHART_ICONS: Record<string, { icon: React.ElementType; label: string }> = {
  'bar-chart': { icon: BarChartIcon, label: 'Bar Chart' },
  'line-chart': { icon: LineChartIcon, label: 'Line Chart' },
  'pie-chart': { icon: PieChartIcon, label: 'Pie Chart' },
  'sankey-chart': { icon: SankeyIcon, label: 'Sankey Chart' },
  'funnel-chart': { icon: FunnelIcon, label: 'Funnel Chart' },
  'segmented-matrix': { icon: MatrixIcon, label: 'Matrix' },
};

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

export function InlineMetric({ metricData, headerEnd, footer, additionalActions = [] }: InlineMetricProps) {
  const theme = useTheme();
  const isDark = theme.palette.mode === 'dark';
  const [response, setResponse] = useState<unknown>(metricData.response);
  const [isLoading, setIsLoading] = useState(!metricData.response);
  const [showQuery, setShowQuery] = useState(false);
  const [showDebugConfig, setShowDebugConfig] = useState(false);
  const [selectedChartId, setSelectedChartId] = useState<string>('table');
  const isFirstLoadRef = useRef(true);
  const hasFetchedRef = useRef(false);
  const userSelectedRef = useRef(false);
  const { debugMode } = useDebugMode();

  // Sync response when metricData.response changes (e.g., when updated via tool call)
  useEffect(() => {
    if (metricData.response) {
      setResponse(metricData.response);
      setIsLoading(false);
    }
  }, [metricData.response]);

  // Fetch data on mount if response is not provided
  useEffect(() => {
    if (!metricData.response && !hasFetchedRef.current) {
      hasFetchedRef.current = true;
      setIsLoading(true);
      executeQuery(metricData.query)
        .then((result) => {
          setResponse(result);
        })
        .catch((error) => {
          console.error('Failed to fetch metric data:', error);
          setResponse({ error: error instanceof Error ? error.message : 'Failed to fetch data' });
        })
        .finally(() => {
          setIsLoading(false);
        });
    }
  }, [metricData.query, metricData.response]);

  const responseData = response as QueryResponse | null;
  const hasError = !!responseData?.error;

  // Handle both wrapped response (QueryResponse) and direct data array
  const data = Array.isArray(response)
    ? response
    : responseData?.response?.data as Record<string, unknown>[] | undefined;
  const statistics = responseData?.response?.statistics as QueryStatistics | undefined;

  // Build view options
  const availableViews = (() => {
    const configuredTypes = getConfiguredChartTypes(metricData.chartConfig);
    if (configuredTypes.length > 0) {
      // Show configured charts
      const configuredViews = configuredTypes.map((chartType) => ({
        id: chartType,
        icon: CHART_ICONS[chartType]?.icon || BarChartIcon,
        label: CHART_ICONS[chartType]?.label || chartType,
      }));
      return [...configuredViews, { id: 'table', icon: TableIcon, label: 'Table' }];
    }
    return [{ id: 'table', icon: TableIcon, label: 'Table' }];
  })();

  // Auto-select first available chart on initial mount only
  if (availableViews.length > 1 && selectedChartId === 'table' && isFirstLoadRef.current) {
    // set firstLoadRef to false
    isFirstLoadRef.current = false;
    const firstChart = availableViews.find(v => v.id !== 'table');
    if (firstChart) {
      // Use setTimeout to avoid render-during-render
      setTimeout(() => setSelectedChartId(firstChart.id), 0);
    }
  }

  const handleDownload = (format: 'csv' | 'json') => {
    if (!data || data.length === 0) return;
    const filename = `metric_${metricData.label.replace(/\s+/g, '_').toLowerCase()}_${Date.now()}`;
    if (format === 'csv') {
      downloadAsCSV(data, filename);
    } else {
      downloadAsJSON(data, filename);
    }
  };

  const actions: ActionItem[] = [
    {
      id: 'query',
      label: 'View Query',
      icon: TerminalIcon,
      onClick: () => setShowQuery(true),
    },
    ...(debugMode && metricData.chartConfig ? [{
      id: 'config',
      label: 'Chart Config',
      icon: BugIcon,
      onClick: () => setShowDebugConfig(true),
    }] : []),
    {
      id: 'csv',
      label: 'Download CSV',
      icon: FileSpreadsheet,
      onClick: () => handleDownload('csv'),
      disabled: !data || data.length === 0,
    },
    {
      id: 'json',
      label: 'Download JSON',
      icon: FileJson,
      onClick: () => handleDownload('json'),
      disabled: !data || data.length === 0,
    },
    ...(additionalActions.length > 0
      ? [
          { id: 'separator', label: '', icon: TerminalIcon, onClick: () => {}, disabled: true },
          ...additionalActions,
        ]
      : []),
  ];

  const titleTooltip = metricData.description
    ? `${metricData.label}\n\n${metricData.description}`
    : metricData.label;

  return (
    <Paper
      elevation={0}
      sx={{
        mt: 1.5,
        border: '1px solid rgba(133, 129, 129,0.5)',
        borderRadius: 2,
        overflow: 'hidden',
        bgcolor: 'background.paper',
        flex: 1,
        width: '100%'
      }}
    >
      {/* Header */}
      <Box
        sx={{
          px: 2,
          py: 1.5,
          borderBottom: 1,
          borderColor: 'divider',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        {metricData.label && (
          <Tooltip
            title={
              metricData.description ? (
                <Box sx={{ p: 0.5 }}>
                  <Typography variant="body2" fontWeight={600}>
                    {metricData.label}
                  </Typography>
                  <Typography variant="body2" sx={{ mt: 0.5 }}>
                    {metricData.description}
                  </Typography>
                </Box>
              ) : (
                metricData.label
              )
            }
            PopperProps={{
              modifiers: [
                {
                  name: 'preventOverflow',
                  options: {
                    altAxis: true,
                    padding: 8,
                  },
                },
              ],
            }}
          >
            <Typography
              variant="subtitle2"
              fontWeight={600}
              sx={{
                cursor: 'help',
              }}
            >
              {metricData.label}
            </Typography>
          </Tooltip>
        )}
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
          <ActionsDropdown actions={actions} />
          {headerEnd}
        </Box>
      </Box>

      {/* Query Dialog */}
      <Dialog open={showQuery} onClose={() => setShowQuery(false)} maxWidth="md" fullWidth>
        <DialogTitle sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <Typography variant="subtitle1" fontWeight={600}>
            SQL Query
          </Typography>
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Tooltip title="Copy query">
              <IconButton
                size="small"
                onClick={() => {
                  navigator.clipboard.writeText(metricData.query);
                }}
              >
                <CopyIcon size={16} />
              </IconButton>
            </Tooltip>
            <IconButton size="small" onClick={() => setShowQuery(false)}>
              <CloseIcon size={16} />
            </IconButton>
          </Box>
        </DialogTitle>
        <DialogContent sx={{ p: 0 }}>
          <SyntaxHighlighter
            language="sql"
            style={isDark ? oneDark : oneLight}
            customStyle={{
              margin: 0,
              borderRadius: 0,
              fontSize: '0.85rem',
              maxHeight: '60vh',
            }}
            showLineNumbers
          >
            {metricData.query}
          </SyntaxHighlighter>
        </DialogContent>
      </Dialog>

      {/* Results */}
      <Box sx={{ p: 2 }}>
        {/* Chart Type Selector */}
        {availableViews.length > 1 && (
          <Box sx={{ mb: 2 }}>
            <ToggleButtonGroup
              value={selectedChartId}
              exclusive
              onChange={(_, value) => {
                if (value) {
                  userSelectedRef.current = true;
                  setSelectedChartId(value);
                }
              }}
              size="small"
            >
              {availableViews.map((view) => {
                const Icon = view.icon;
                return (
                  <ToggleButton key={view.id} value={view.id} sx={{ px: 1 }}>
                    <Icon size={16} />
                  </ToggleButton>
                );
              })}
            </ToggleButtonGroup>
          </Box>
        )}

        {/* Loading State */}
        {isLoading && (
          <Box>
            <Skeleton variant="rectangular" height={200} sx={{ mb: 1 }} />
            <Skeleton variant="text" width="60%" />
          </Box>
        )}

        {/* Error State */}
        {!isLoading && hasError && (
          <Box sx={{ color: 'error.main', fontSize: '0.875rem' }}>
            {responseData?.error}
          </Box>
        )}

        {/* Chart View */}
        {!isLoading && !hasError && selectedChartId !== 'table' && data && (
          <Box>
            {(() => {
              const selectedChartType = selectedChartId as ChartType;
              const selectedChartConfig = getChartConfig(metricData.chartConfig, selectedChartType);

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
                  <Typography variant="body2" color="text.secondary">
                    This chart type is not compatible with the current data structure.
                  </Typography>
                );
              }
              // Auto-switch to table on first render if incompatible
              setTimeout(() => setSelectedChartId('table'), 0);
              return null;
            })()}
          </Box>
        )}

        {/* Table View */}
        {!isLoading && !hasError && selectedChartId === 'table' && data && (
          <Box
            component="table"
            sx={{
              width: '100%',
              borderCollapse: 'collapse',
              fontSize: '0.8rem',
              fontFamily: "'DM Sans', sans-serif",
            }}
          >
            <thead>
              <tr>
                {data.length > 0 && Object.keys(data[0]).map((key) => (
                  <Box
                    key={key}
                    component="th"
                    sx={{
                      textAlign: 'left',
                      p: 1,
                      borderBottom: 1,
                      borderColor: 'divider',
                      fontWeight: 600,
                      bgcolor: 'action.hover',
                    }}
                  >
                    {key}
                  </Box>
                ))}
              </tr>
            </thead>
            <tbody>
              {data.slice(0, 50).map((row, idx) => (
                <tr key={idx}>
                  {Object.values(row).map((val, cellIdx) => (
                    <Box
                      key={cellIdx}
                      component="td"
                      sx={{
                        p: 1,
                        borderBottom: 1,
                        borderColor: 'divider',
                      }}
                    >
                      {isExpandableValue(val) ? <ExpandableCellValue value={val} /> : formatNumber(val)}
                    </Box>
                  ))}
                </tr>
              ))}
            </tbody>
          </Box>
        )}

        {/* Show more rows indicator */}
        {!isLoading && !hasError && data && data.length > 50 && (
          <Typography variant="caption" color="text.secondary" sx={{ mt: 1, display: 'block' }}>
            Showing 50 of {data.length} rows. Download for full data.
          </Typography>
        )}
      </Box>

      {/* Debug: Chart Config Dialog */}
      <Dialog open={showDebugConfig} onClose={() => setShowDebugConfig(false)} maxWidth="md" fullWidth>
        <DialogTitle sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <Typography variant="subtitle1" fontWeight={600}>
            Chart Config
          </Typography>
          <IconButton size="small" onClick={() => setShowDebugConfig(false)}>
            <CloseIcon size={16} />
          </IconButton>
        </DialogTitle>
        <DialogContent sx={{ p: 0 }}>
          <SyntaxHighlighter
            language="json"
            style={isDark ? oneDark : oneLight}
            customStyle={{
              margin: 0,
              borderRadius: 0,
              fontSize: '0.8rem',
              maxHeight: '60vh',
            }}
            showLineNumbers
          >
            {JSON.stringify(metricData.chartConfig, null, 2)}
          </SyntaxHighlighter>
        </DialogContent>
      </Dialog>

      {/* Footer */}
      {footer && (
        <Box
          sx={{
            px: 2,
            py: 1,
            borderTop: '1px solid',
            borderColor: 'divider',
            display: 'flex',
            justifyContent: 'flex-end',
            alignItems: 'center',
          }}
        >
          {footer}
        </Box>
      )}
    </Paper>
  );
}
