import { useState, useRef, useCallback, useEffect, memo } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark, oneLight } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { Box, IconButton, Typography, Skeleton, useTheme, Tooltip, Dialog, DialogContent, DialogTitle, DialogActions, Button, TextField, Stack, ToggleButton, ToggleButtonGroup, Select, MenuItem, Chip, Slide, Paper } from '@mui/material';
import { X as CloseIcon, GripVertical as DragHandleIcon, Maximize as ExpandIcon, Minimize as CollapseIcon, RefreshCw as RefreshIcon, Terminal as TerminalIcon, Play as RunIcon, Bug as DebugIcon } from 'lucide-react';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { DateTimePicker } from '@mui/x-date-pickers/DateTimePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { ResultPanel } from '../MetricInstance/ResultPanel';
import { SearchableDropdown } from './SearchableDropdown';
import type { DynamicQuery, SavedMetric } from '../../types/metric';
import type { ChartConfig } from '../../types/chart';
import { executeQuery } from '../../services/clickhouseService';
import { substituteVariables } from '../../services/dynamicQueryService';
import { trackMetricRefreshed } from '../../utils/analytics';

const PRIMARY_COLOR = '#137fec';

// Chart type identifiers
const CHART_TYPES = ['bar-chart', 'line-chart', 'pie-chart', 'funnel-chart', 'sankey-chart'] as const;
type ChartType = typeof CHART_TYPES[number];

// Helper to get configured chart types from chartConfig
const getConfiguredChartTypes = (chartConfig: ChartConfig | undefined): ChartType[] => {
  if (!chartConfig) return [];
  return CHART_TYPES.filter((type) => chartConfig[type]);
};

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

interface RightPanelProps {
  isOpen: boolean;
  onClose: () => void;
  query: string;
  response?: unknown;
  label: string;
  description?: string;
  onRefresh?: () => void;
  savedMetricId?: string;
  dynamicQuery?: DynamicQuery;
  chartConfig?: ChartConfig;
}

const DEFAULT_WIDTH = 700;
const MIN_WIDTH = 300;
const MAX_WIDTH = 1200;

// Memoized component for rendering individual variable inputs
// Defined outside to prevent recreation on every render (which causes focus loss)
interface VariableInputProps {
  varKey: string;
  config: DynamicQuery['variables'][string];
  value: string;
  onChange: (key: string, value: string) => void;
  // For searchable dropdown, provide a function to get options
  getDropdownQuery?: (searchValue: string) => Promise<string[]>;
  // For regular dropdowns, provide the options directly
  dropdownOptions?: string[];
}

const VariableInput = memo(({ varKey, config, value, onChange, getDropdownQuery, dropdownOptions }: VariableInputProps) => {
  const hasDropdown = config.dropDownQuery !== undefined;
  const isSearchableDropdown = config.inputType === 'searchable_dropdown';

  return (
    <Box sx={{ minWidth: 0, flex: config.inputType === 'direction' ? 'none' : 1 }}>
      <Typography variant="caption" color="text.secondary" sx={{ mb: 0.5, display: 'block' }}>
        {config.description}
      </Typography>
      {config.inputType === 'direction' ? (
        <ToggleButtonGroup
          value={value}
          exclusive
          onChange={(e, newValue) => {
            if (newValue !== null) {
              onChange(varKey, newValue);
            }
          }}
          size="small"
          sx={{ gap: 0.5 }}
        >
          <ToggleButton value="ASC" sx={{ px: 1.5, py: 0.75, textTransform: 'none', height: 32 }}>
            Asc
          </ToggleButton>
          <ToggleButton value="DESC" sx={{ px: 1.5, py: 0.75, textTransform: 'none', height: 32 }}>
            Desc
          </ToggleButton>
        </ToggleButtonGroup>
      ) : isSearchableDropdown ? (
        <SearchableDropdown
          value={value || ''}
          onChange={(newValue) => onChange(varKey, newValue)}
          getQuery={getDropdownQuery || (() => Promise.resolve([]))}
          label={config.description}
          height={32}
        />
      ) : hasDropdown ? (
        <Select
          value={value || ''}
          onChange={(e) => onChange(varKey, e.target.value as string)}
          displayEmpty
          size="small"
          fullWidth
          MenuProps={{
            PaperProps: {
              sx: {
                maxHeight: 300,
              },
            },
            // Ensure dropdown is positioned correctly
            anchorOrigin: {
              vertical: 'bottom',
              horizontal: 'left',
            },
            transformOrigin: {
              vertical: 'top',
              horizontal: 'left',
            },
          }}
          sx={{
            height: 32,
            fontSize: '0.875rem',
            '& .MuiSelect-select': {
              py: 0.5,
            },
          }}
        >
          <MenuItem value="" sx={{ fontSize: '0.875rem' }}>
            <em>Select...</em>
          </MenuItem>
          {(dropdownOptions || []).map((option) => (
            <MenuItem key={option} value={option} sx={{ fontSize: '0.875rem' }}>
              {option}
            </MenuItem>
          ))}
        </Select>
      ) : config.inputType === 'date' ? (
        <DatePicker
          value={value ? new Date(value) : null}
          onChange={(date) => {
            if (date instanceof Date && !isNaN(date.getTime())) {
              onChange(varKey, date.toISOString().split('T')[0]);
            } else {
              onChange(varKey, '');
            }
          }}
          slotProps={{
            textField: {
              size: 'small',
              fullWidth: true,
            },
          }}
        />
      ) : config.inputType === 'datetime' ? (
        <DateTimePicker
          value={value ? new Date(value) : null}
          onChange={(date) => {
            if (date instanceof Date && !isNaN(date.getTime())) {
              onChange(varKey, date.toISOString());
            } else {
              onChange(varKey, '');
            }
          }}
          slotProps={{
            textField: {
              size: 'small',
              fullWidth: true,
              sx: {
                '& .MuiInputBase-root': {
                  height: 32,
                  minHeight: 32,
                },
                '& .MuiInputBase-input': {
                  fontSize: '0.875rem',
                  height: 32,
                  padding: '4px 8px',
                },
                '& .MuiOutlinedInput-root': {
                  height: 32,
                },
              },
            },
          }}
        />
      ) : (
        <TextField
          fullWidth
          size="small"
          type={config.inputType === 'number' ? 'number' : 'text'}
          value={value}
          onChange={(e) => onChange(varKey, e.target.value)}
          InputProps={{
            sx: { fontSize: '0.875rem' },
          }}
          sx={{
            '& .MuiOutlinedInput-input': {
              py: 0.75,
              height: 32,
            },
          }}
        />
      )}
    </Box>
  );
});

VariableInput.displayName = 'VariableInput';

// Custom comparison function for memo
const areEqual = (prevProps: VariableInputProps, nextProps: VariableInputProps) => {
  return (
    prevProps.varKey === nextProps.varKey &&
    prevProps.value === nextProps.value &&
    prevProps.config === nextProps.config &&
    prevProps.getDropdownQuery === nextProps.getDropdownQuery &&
    JSON.stringify(prevProps.dropdownOptions) === JSON.stringify(nextProps.dropdownOptions)
  );
};

const MemoVariableInput = memo(VariableInput, areEqual);

export function RightPanel({ isOpen, onClose, query, response: responseProp, label, description, onRefresh, savedMetricId, dynamicQuery: dynamicQueryProp, chartConfig }: RightPanelProps) {
  const theme = useTheme();
  const isDark = theme.palette.mode === 'dark';
  // Initialize to first chart from chartConfig if available, otherwise table
  const configuredTypes = getConfiguredChartTypes(chartConfig);
  const initialSelectedChartId = configuredTypes.length > 0 ? configuredTypes[0] : 'table';
  const [selectedChartId, setSelectedChartId] = useState(initialSelectedChartId);
  const [panelWidth, setPanelWidth] = useState(DEFAULT_WIDTH);
  const [isDragging, setIsDragging] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [showQueryDialog, setShowQueryDialog] = useState(false);
  const [showDebugView, setShowDebugView] = useState(false);
  const [variableValues, setVariableValues] = useState<Record<string, string>>({});
  const [dynamicResponse, setDynamicResponse] = useState<unknown | null>(null);
  const [isRunningDynamicQuery, setIsRunningDynamicQuery] = useState(false);
  const [dynamicQuery, setDynamicQuery] = useState<DynamicQuery | undefined>(undefined);
  const [dropdownOptions, setDropdownOptions] = useState<Record<string, string[]>>({});
  const [response, setResponse] = useState<unknown | undefined>(responseProp);
  const [isLoadingResponse, setIsLoadingResponse] = useState(!responseProp);
  const startXRef = useRef<number>(0);
  const startWidthRef = useRef<number>(DEFAULT_WIDTH);
  const hasInitialQueryRun = useRef(false);
  const hasFetchedResponseRef = useRef(false);

  // Fetch response on mount if not provided and no dynamicQuery
  useEffect(() => {
    if (!responseProp && !dynamicQueryProp && !hasFetchedResponseRef.current && isOpen) {
      hasFetchedResponseRef.current = true;
      setIsLoadingResponse(true);
      executeQuery(query)
        .then((result) => {
          setResponse(result);
        })
        .catch((error) => {
          console.error('Failed to fetch response:', error);
          setResponse({ error: error instanceof Error ? error.message : 'Failed to fetch data' });
        })
        .finally(() => {
          setIsLoadingResponse(false);
        });
    }
  }, [query, responseProp, dynamicQueryProp, isOpen]);

  // Look up saved metric from storage when savedMetricId changes
  useEffect(() => {
    if (savedMetricId) {
      try {
        const data = localStorage.getItem('hyper_analytics_saved_metrics');
        const metrics: SavedMetric[] = data ? JSON.parse(data) : [];
        const metric = metrics.find(m => m.id === savedMetricId);
        console.log('[RightPanel] Looking up saved metric:', savedMetricId, 'found:', metric, 'has dynamicQuery:', !!metric?.dynamicQuery);
        if (metric?.dynamicQuery) {
          console.log('[RightPanel] Setting dynamicQuery:', metric.dynamicQuery);
          setDynamicQuery(metric.dynamicQuery);
        }
      } catch (error) {
        console.error('Failed to look up saved metric:', error);
      }
    } else if (dynamicQueryProp) {
      // Use prop directly from chat flow
      console.log('[RightPanel] Using dynamicQueryProp:', dynamicQueryProp);
      setDynamicQuery(dynamicQueryProp);
    } else {
      setDynamicQuery(undefined);
    }
  }, [savedMetricId, dynamicQueryProp]);

  // Initialize variable values from dynamicQuery defaults
  useEffect(() => {
    console.log('[RightPanel] dynamicQuery changed:', dynamicQuery);
    if (dynamicQuery) {
      const initialValues: Record<string, string> = {};
      Object.entries(dynamicQuery.variables).forEach(([key, config]) => {
        initialValues[key] = config.default;
      });
      console.log('[RightPanel] Setting variable values:', initialValues);
      setVariableValues(initialValues);
      setDynamicResponse(null);
      hasInitialQueryRun.current = false;
    }
  }, [dynamicQuery]);

  // Load dropdown options for regular dropdowns (not searchable)
  useEffect(() => {
    if (!dynamicQuery) return;

    const loadDropdownOptions = async () => {
      const newDropdownOptions: Record<string, string[]> = {};

      for (const [key, config] of Object.entries(dynamicQuery.variables)) {
        const hasDropdown = config.dropDownQuery !== undefined;
        const isSearchableDropdown = config.inputType === 'searchable_dropdown';

        // Only load regular dropdowns here, searchable dropdowns handle their own loading
        if (hasDropdown && !isSearchableDropdown) {
          try {
            const response = await executeQuery(config.dropDownQuery);
            // Handle the response structure: { response: { data: [{ column: value }] } }
            const responseData = (response as any)?.response?.data;
            const dataArray = Array.isArray(responseData) ? responseData : Array.isArray(response) ? response : [];
            // Extract the first column from the result
            const options = dataArray.map((row: unknown) => {
              const rowObj = row as Record<string, unknown>;
              const firstKey = Object.keys(rowObj)[0];
              const val = String(rowObj[firstKey] ?? '');
              return val;
            }).filter(v => v !== '' && v !== 'null');
            newDropdownOptions[key] = options;
          } catch (error) {
            console.error(`Failed to load dropdown options for ${key}:`, error);
            newDropdownOptions[key] = [];
          }
        }
      }

      setDropdownOptions(newDropdownOptions);
    };

    loadDropdownOptions();
  }, [dynamicQuery]);

  // Handle variable value changes
  const handleVariableChange = (key: string, value: string) => {
    const config = dynamicQuery?.variables[key];
    // Don't trim or replace empty for date/datetime/searchable_dropdown inputs
    const shouldPreserveEmpty =
      config?.inputType === 'date' ||
      config?.inputType === 'datetime' ||
      config?.inputType === 'searchable_dropdown';
    const finalValue = (!shouldPreserveEmpty && value.trim() === '')
      ? (config?.default ?? '')
      : value;
    setVariableValues(prev => ({
      ...prev,
      [key]: finalValue,
    }));
  };

  // Execute query with substituted variables
  const handleRunDynamicQuery = useCallback(async () => {
    if (!dynamicQuery) return;

    setIsRunningDynamicQuery(true);
    setDynamicResponse(null);

    try {
      // Use default values for empty inputs
      const valuesWithDefaults: Record<string, string> = {};
      Object.entries(dynamicQuery.variables).forEach(([key, config]) => {
        const currentValue = variableValues[key];
        valuesWithDefaults[key] = currentValue?.trim() === '' ? config.default : currentValue ?? config.default;
      });

      const substitutedQuery = substituteVariables(valuesWithDefaults, dynamicQuery);
      const result = await executeQuery(substitutedQuery);
      setDynamicResponse(result);
    } catch (error) {
      console.error('Failed to execute dynamic query:', error);
      setDynamicResponse({ error: error instanceof Error ? error.message : 'Failed to execute query' });
    } finally {
      setIsRunningDynamicQuery(false);
    }
  }, [dynamicQuery, variableValues]);

  // Auto-run dynamic query with default values when dynamicQuery is available
  useEffect(() => {
    if (dynamicQuery && Object.keys(variableValues).length > 0 && dynamicResponse === null && !hasInitialQueryRun.current) {
      hasInitialQueryRun.current = true;
      handleRunDynamicQuery();
    }
  }, [dynamicQuery, variableValues, dynamicResponse, handleRunDynamicQuery]);

  // Function to get dropdown options for searchable dropdowns
  const getDropdownQuery = useCallback(async (varKey: string, searchValue: string): Promise<string[]> => {
    if (!dynamicQuery) return [];

    const config = dynamicQuery.variables[varKey];
    if (!config?.dropDownQuery) return [];

    try {
      let query = config.dropDownQuery;

      // Replace {varKey} placeholder with search value
      // The template should already have % signs and quotes, e.g., LIKE '%{event_name}%'
      const placeholder = `{${varKey}}`;
      const formattedValue = searchValue.replace(/'/g, "''");
      query = query.replace(new RegExp(placeholder, 'g'), formattedValue);

      // Also replace any other variables with their current values
      Object.entries(dynamicQuery.variables).forEach(([otherKey, otherConfig]) => {
        if (otherKey !== varKey) {
          const otherPlaceholder = `{${otherKey}}`;
          const otherValue = variableValues[otherKey] ?? otherConfig.default;
          query = query.replace(new RegExp(otherPlaceholder, 'g'), otherValue);
        }
      });

      const response = await executeQuery(query);
      // Handle the response structure: { response: { data: [{ column: value }] } }
      const responseData = (response as any)?.response?.data;
      if (!responseData || !Array.isArray(responseData)) {
        return [];
      }

      // Extract the first column from each row
      const options = responseData.map((row: Record<string, unknown>) => {
        const firstKey = Object.keys(row)[0];
        const val = String(row[firstKey] ?? '');
        return val;
      }).filter(v => v !== '' && v !== 'null');

      return options;
    } catch (error) {
      console.error(`Failed to load dropdown options for ${varKey}:`, error);
      return [];
    }
  }, [dynamicQuery, variableValues]);

  // Derive loading state from response - loading when response is null and panel is open
  const isLoading = isOpen && isLoadingResponse;

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    startXRef.current = e.clientX;
    startWidthRef.current = panelWidth;

    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }, [panelWidth]);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isDragging) return;

    const deltaX = startXRef.current - e.clientX;
    const newWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidthRef.current + deltaX));
    setPanelWidth(newWidth);
  }, [isDragging]);

  const handleMouseUp = useCallback(() => {
    if (isDragging) {
      setIsDragging(false);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
  }, [isDragging]);

  const handleToggleFullscreen = () => {
    setIsFullscreen(!isFullscreen);
  };

  useEffect(() => {
    if (isDragging) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
      return () => {
        window.removeEventListener('mousemove', handleMouseMove);
        window.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [isDragging, handleMouseMove, handleMouseUp]);

  // Close fullscreen when panel is closed
  useEffect(() => {
    if (!isOpen) {
      setIsFullscreen(false);
    }
  }, [isOpen]);

  // Reset selectedChartId when chartConfig changes (e.g., opening new metric)
  useEffect(() => {
    const types = getConfiguredChartTypes(chartConfig);
    if (types.length > 0) {
      setSelectedChartId(types[0]);
    } else {
      setSelectedChartId('table');
    }
  }, [chartConfig]);

  if (!isOpen) {
    return null;
  }

  // Get values with defaults for substitution (used for display and query execution)
  const getValuesWithDefaults = (): Record<string, string> => {
    if (!dynamicQuery) return {};
    const values: Record<string, string> = {};
    Object.entries(dynamicQuery.variables).forEach(([key, config]) => {
      const currentValue = variableValues[key];
      values[key] = currentValue?.trim() === '' ? config.default : currentValue ?? config.default;
    });
    return values;
  };

  const instance: MetricInstance = {
    id: 'chat-result-panel',
    metricId: null,
    name: label,
    description,
    query: dynamicQuery ? substituteVariables(getValuesWithDefaults(), dynamicQuery) : query,
    chart: '',
    variables: {},
    functions: {},
    isDirty: false,
    response: dynamicResponse !== null ? dynamicResponse : response,
    isRunning: isRunningDynamicQuery,
    showRaw: true,
    selectedChartId,
    panelOrientation: 'horizontal',
    error: null,
    chartConfig,
  };

  const handleUpdate = (updates: Partial<MetricInstance>) => {
    if (updates.selectedChartId) {
      setSelectedChartId(updates.selectedChartId);
    }
  };

  const renderPanelContent = () => (
    <LocalizationProvider dateAdapter={AdapterDateFns}>
      <>
      {/* Resize handle - only show in normal mode */}
      {!isFullscreen && (
        <Box
          onMouseDown={handleMouseDown}
          sx={{
            position: 'absolute',
            left: 0,
            top: 0,
            bottom: 0,
            width: 4,
            cursor: 'col-resize',
            zIndex: 20,
            bgcolor: isDragging ? 'primary.main' : 'transparent',
            '&:hover': {
              bgcolor: 'primary.main',
            },
            transition: isDragging ? 'none' : 'background-color 0.2s',
          }}
        >
          {/* Visual indicator */}
          <Box
            sx={{
              position: 'absolute',
              left: 1,
              top: '50%',
              transform: 'translateY(-50%)',
              display: 'flex',
              flexDirection: 'column',
              gap: 1,
              opacity: isDragging ? 1 : 0,
              '&:hover': {
                opacity: 1,
              },
            }}
          >
            {[...Array(4)].map((_, i) => (
              <DragHandleIcon
                key={i}
                size={8}
                style={{
                  color: 'white',
                  transform: 'rotate(90deg)',
                }}
              />
            ))}
          </Box>
        </Box>
      )}

      {/* Panel */}
      <Box
        sx={{
          width: isFullscreen ? '100%' : panelWidth,
          height: '100%',
          bgcolor: isDark ? 'rgba(0,0,0,0.1)' : 'rgba(0,0,0,0.02)',
          borderLeft: 1,
          borderColor: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.08)',
          display: 'flex',
          flexDirection: 'column',
          zIndex: 10,
          flexShrink: 0,
          fontSize: '0.875rem',
          animation: 'slideInFromRight 0.3s ease-out',
          '@keyframes slideInFromRight': {
            from: {
              opacity: 0,
              transform: 'translateX(20px)',
            },
            to: {
              opacity: 1,
              transform: 'translateX(0)',
            },
          },
        }}
      >
        <Box
          sx={{
            px: 2,
            py: 1.5,
            borderBottom: 1,
            borderColor: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.08)',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            bgcolor: isDark ? 'rgba(255,255,255,0.05)' : 'rgba(255,255,255,0.9)',
            cursor: 'move',
          }}
          onMouseDown={(e) => {
            // Allow dragging from header too
            if ((e.target as HTMLElement).closest('.MuiIconButton-root')) return;
            handleMouseDown(e);
          }}
        >
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Typography variant="subtitle2" fontWeight={600}>
              {label}
            </Typography>
            {onRefresh && (
              <Tooltip title="Refresh metric">
                <IconButton
                  onClick={() => {
                    const queryHash = btoa(query.substring(0, 100)).substring(0, 16);
                    trackMetricRefreshed(queryHash);
                    onRefresh();
                  }}
                  size="small"
                  className="MuiIconButton-root"
                  sx={{
                    '&:hover': { bgcolor: 'rgba(0,0,0,0.04)' },
                  }}
                >
                  <RefreshIcon size={16} />
                </IconButton>
              </Tooltip>
            )}
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
            <Tooltip title="View query">
              <IconButton
                onClick={() => setShowQueryDialog(true)}
                size="small"
                className="MuiIconButton-root"
                sx={{
                  '&:hover': { bgcolor: 'rgba(0,0,0,0.04)' },
                }}
              >
                <TerminalIcon size={16} />
              </IconButton>
            </Tooltip>
            <Tooltip title={isFullscreen ? 'Collapse to sidebar' : 'Expand to fullscreen'}>
              <IconButton
                onClick={handleToggleFullscreen}
                size="small"
                className="MuiIconButton-root"
                sx={{
                  '&:hover': { bgcolor: 'rgba(0,0,0,0.04)' },
                }}
              >
                {isFullscreen ? <CollapseIcon size={16} /> : <ExpandIcon size={16} />}
              </IconButton>
            </Tooltip>
            <IconButton
              onClick={onClose}
              size="small"
              className="MuiIconButton-root"
              sx={{
                '&:hover': { bgcolor: 'rgba(0,0,0,0.04)' },
              }}
            >
              <CloseIcon size={16} />
            </IconButton>
          </Box>
        </Box>

        <Box sx={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
          {/* Dynamic Query Variables Form */}
          {(() => {
            console.log('[RightPanel] Rendering dynamicQuery form, dynamicQuery:', dynamicQuery);
            return dynamicQuery;
          })() && (
            <Paper
              sx={{
                p: 2,
                pb: 1.5,
                m: 1.5,
              }}
            >
              <Typography variant="caption" fontWeight={600} sx={{ mb: 1.5, display: 'block' }}>
                Parameters
              </Typography>

              {/* If layout is provided, render rows */}
              {dynamicQuery.layout ? (
                <>
                  <Stack spacing={1}>
                    {dynamicQuery.layout.map((row, rowIndex) => (
                      <Box key={rowIndex} sx={{ display: 'flex', gap: 1 }}>
                        {row.map((varKey) => {
                          const config = dynamicQuery.variables[varKey];
                          if (!config) return null;
                          return (
                            <MemoVariableInput
                              key={varKey}
                              varKey={varKey}
                              config={config}
                              value={variableValues[varKey] ?? config.default}
                              onChange={handleVariableChange}
                              getDropdownQuery={(searchValue) => getDropdownQuery(varKey, searchValue)}
                              dropdownOptions={dropdownOptions[varKey]}
                            />
                          );
                        })}
                      </Box>
                    ))}
                  </Stack>
                  {/* Run Metric button */}
                  <Box sx={{ mt: 1.5, display: 'flex', justifyContent: 'flex-end' }}>
                    <Button
                      variant="contained"
                      size="small"
                      startIcon={<RunIcon />}
                      onClick={handleRunDynamicQuery}
                      disabled={isRunningDynamicQuery}
                      sx={{
                        bgcolor: PRIMARY_COLOR,
                        '&:hover': { bgcolor: '#0d6fd6' },
                        py: 0.75,
                      }}
                    >
                      {isRunningDynamicQuery ? 'Running...' : 'Run Metric'}
                    </Button>
                  </Box>
                </>
              ) : (
                /* Fallback: render all variables in single column */
                <>
                  <Stack spacing={1}>
                    {Object.entries(dynamicQuery.variables).map(([key, config]) => (
                      <MemoVariableInput
                        key={key}
                        varKey={key}
                        config={config}
                        value={variableValues[key] ?? config.default}
                        onChange={handleVariableChange}
                        getDropdownQuery={(searchValue) => getDropdownQuery(key, searchValue)}
                        dropdownOptions={dropdownOptions[key]}
                      />
                    ))}
                  </Stack>
                  {/* Run Metric button */}
                  <Box sx={{ mt: 1.5, display: 'flex', justifyContent: 'flex-end' }}>
                    <Button
                      variant="contained"
                      size="small"
                      startIcon={<RunIcon />}
                      onClick={handleRunDynamicQuery}
                      disabled={isRunningDynamicQuery}
                      sx={{
                        bgcolor: PRIMARY_COLOR,
                        '&:hover': { bgcolor: '#0d6fd6' },
                        py: 0.75,
                      }}
                    >
                      {isRunningDynamicQuery ? 'Running...' : 'Run Metric'}
                    </Button>
                  </Box>
                </>
              )}
            </Paper>
          )}

          {isLoading && !dynamicResponse ? (
            <Box sx={{ p: 2 }}>
              <Skeleton variant="text" width="60%" height={24} sx={{ mb: 2 }} />
              <Skeleton variant="rectangular" width="100%" height={200} sx={{ mb: 2 }} />
              <Skeleton variant="text" width="100%" height={40} />
              <Skeleton variant="text" width="100%" height={40} />
              <Skeleton variant="text" width="80%" height={40} />
            </Box>
          ) : (
            <Box sx={{ flex: 1, overflow: 'hidden', minHeight: 0, m: 1.5, display: 'flex', flexDirection: 'column', animation: 'fadeIn 0.3s ease-out', '@keyframes fadeIn': { from: { opacity: 0 }, to: { opacity: 1 } } }}>
              <ResultPanel instance={instance} onUpdate={handleUpdate} />
            </Box>
          )}
        </Box>
      </Box>
    </>
    </LocalizationProvider>
  );

  // Fullscreen dialog mode
  if (isFullscreen) {
    return (
      <Dialog
        open={isOpen}
        onClose={onClose}
        maxWidth={false}
        fullWidth
        PaperProps={{
          sx: {
            height: '95vh',
            width: '95vw',
            maxHeight: '95vh',
            maxWidth: '95vw',
            margin: 0,
            borderRadius: 2,
          },
        }}
      >
        <DialogContent sx={{ p: 0, height: '100%', overflow: 'hidden' }}>
          {renderPanelContent()}
        </DialogContent>
      </Dialog>
    );
  }

  // Normal sidebar mode
  return (
<>
      <Box sx={{ height: '100%' }}>
      {renderPanelContent()}
      </Box>
     <Dialog
      open={showQueryDialog}
        onClose={() => {
          setShowQueryDialog(false);
          setShowDebugView(false);
        }}
        maxWidth="lg"
        fullWidth
      >
        <DialogTitle sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span>{showDebugView ? 'Debug Info' : 'Query'}</span>
          {dynamicQuery && (
            <Tooltip title={showDebugView ? 'Show Query' : 'Show Debug Info'}>
              <IconButton
                size="small"
                onClick={() => setShowDebugView(!showDebugView)}
                sx={{ ml: 1 }}
              >
                <DebugIcon size={16} />
              </IconButton>
            </Tooltip>
          )}
        </DialogTitle>
        <DialogContent sx={{ pt: 1 }}>
          {showDebugView && dynamicQuery ? (
            <Stack spacing={2}>
              {/* Template Query */}
              <Box>
                <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1 }}>
                  Template Query
                </Typography>
                <SyntaxHighlighter
                  language="sql"
                  style={isDark ? oneDark : oneLight}
                  customStyle={{
                    margin: 0,
                    borderRadius: 2,
                    maxHeight: '20vh',
                    overflow: 'auto',
                    fontFamily: 'monospace',
                    fontSize: '0.875rem'
                  }}
                >
                  {dynamicQuery.query}
                </SyntaxHighlighter>
              </Box>

              {/* Variables */}
              <Box>
                <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1 }}>
                  Variables
                </Typography>
                <Box sx={{ maxHeight: '20vh', overflow: 'auto' }}>
                  {Object.entries(dynamicQuery.variables).map(([key, config]) => (
                    <Box key={key} sx={{ mb: 1.5, p: 1, bgcolor: isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.03)', borderRadius: 1 }}>
                      <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.5 }}>
                        {key}
                      </Typography>
                      <Typography variant="body2" sx={{ mb: 0.5 }}>
                        {config.description}
                      </Typography>
                      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                        <Chip size="small" label={`Type: ${config.inputType}`} />
                        <Chip size="small" label={`Default: ${config.default}`} />
                        {config.dropDownQuery && <Chip size="small" label="Has Dropdown" />}
                      </Box>
                    </Box>
                  ))}
                </Box>
              </Box>

              {/* Layout */}
              {dynamicQuery.layout && (
                <Box>
                  <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1 }}>
                    Layout
                  </Typography>
                  <SyntaxHighlighter
                    language="json"
                    style={isDark ? oneDark : oneLight}
                    customStyle={{
                      margin: 0,
                      borderRadius: 2,
                      maxHeight: '15vh',
                      overflow: 'auto',
                      fontFamily: 'monospace',
                      fontSize: '0.875rem'
                    }}
                  >
                    {JSON.stringify(dynamicQuery.layout, null, 2)}
                  </SyntaxHighlighter>
                </Box>
              )}

              {/* Current Values */}
              <Box>
                <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1 }}>
                  Current Values
                </Typography>
                <Box sx={{ maxHeight: '15vh', overflow: 'auto' }}>
                  {Object.entries(getValuesWithDefaults()).map(([key, value]) => (
                    <Box key={key} sx={{ display: 'flex', justifyContent: 'space-between', py: 0.5, borderBottom: 1, borderColor: isDark ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.1)' }}>
                      <Typography variant="body2">{key}</Typography>
                      <Typography variant="body2" color="text.secondary">{value}</Typography>
                    </Box>
                  ))}
                </Box>
              </Box>
            </Stack>
          ) : (
            <SyntaxHighlighter
              language="sql"
              style={isDark ? oneDark : oneLight}
              customStyle={{
                margin: 0,
                borderRadius: 2,
                maxHeight: '60vh',
                overflow: 'auto',
                fontFamily: 'monospace',
                fontSize: '0.875rem'
              }}
            >
              {dynamicQuery ? substituteVariables(getValuesWithDefaults(), dynamicQuery) : query}
            </SyntaxHighlighter>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => {
            setShowQueryDialog(false);
            setShowDebugView(false);
          }}>Close</Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
