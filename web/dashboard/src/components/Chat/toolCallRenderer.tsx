import type { ReactNode } from 'react';
import { useState } from 'react';
import { Box, Button, CircularProgress } from '@mui/material';
import { Save as SaveIcon } from 'lucide-react';
import { InlineMetric } from './InlineMetric';
import ShinyText from './ShinyText';
import { dashboardService } from '../../services/dashboardService';
import { useToast } from '../../contexts/ToastContext';
import { useProject } from '../../contexts/ProjectContext';
import { ApiError } from '../../services/apiClient';

interface ToolCall {
  function: {
    arguments: string;
    name: string;
  };
  id: string;
}

interface ChatMessageRecord {
  id: string;
  chat_id: string;
  created_at: string;
  message: {
    role?: 'user' | 'assistant' | 'tool';
    content?: string | null;
    tool_calls?: Array<ToolCall>;
    tool_call_id?: string;
  };
  isComplete?: boolean
}

function JsonParse(content: string){
  try{
    return JSON.parse(content);
  } catch (e){}
  return null;
}

// light colored text with italics
function ToolCallMsg({content, isComplete}: {content: string, isComplete?: boolean}){
  if (!isComplete) {
    return <ShinyText text={content} speed={1.5} color="#888888" shineColor="#ffffff" className="text-sm" />;
  }
  return <span style={{ color: '#888888', fontSize: '0.875rem' }}>{content}</span>;
}

function RequestDateTimeRange({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content="Requested date/time range for analytics" isComplete={isComplete} />;
}

function ListGoldenQueries({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content={JsonParse(tool_call.function.arguments)?.message ?? 'Requested list of golden queries'} isComplete={isComplete} />;
}

function GetCurrentDatetime({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content={JsonParse(tool_call.function.arguments)?.message ?? 'Getting current date and time'} isComplete={isComplete} />;
}

function FetchGoldenQuery({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content={JsonParse(tool_call.function.arguments)?.message ?? 'Fetching golden queries'} isComplete={isComplete} />;
}

function GetDatabaseSchema({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content={JsonParse(tool_call.function.arguments)?.message ?? 'Getting database schema'} isComplete={isComplete} />;
}

function ExecuteClickhouseQuery({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content={JsonParse(tool_call.function.arguments)?.message ?? 'Executing query on database'} isComplete={isComplete} />;
}

function GetEventDescriptions({ tool_call, isComplete }: { tool_call: ToolCall; isComplete?: boolean }): ReactNode {
  return <ToolCallMsg content={JsonParse(tool_call.function.arguments)?.message ?? 'Getting event descriptions'} isComplete={isComplete} />;
}

function findPendingDateTimeRange(records: ChatMessageRecord[], currentIndex: number): boolean {
  // Look for any request_datetime_range that comes AFTER the current position
  // and hasn't received a user response yet
  for (let i = currentIndex; i < records.length; i++) {
    const record = records[i];

    // If we hit a user message after current position, datetime range is resolved
    if (record.message.role === 'user') {
      return false;
    }

    // Check for request_datetime_range tool call
    if (record.message.tool_calls) {
      const hasDateRangeCall = record.message.tool_calls.some(
        (call) => call.function.name === 'request_datetime_range'
      );

      if (hasDateRangeCall) {
        // Found a datetime range request after current position
        // Check if user has responded after this request
        for (let j = i + 1; j < records.length; j++) {
          if (records[j].message.role === 'user') {
            return false;
          }
        }
        return true;
      }
    }
  }

  return false;
}

function PresentMetricInsight({
  tool_call,
  records,
  currentIndex,
  isComplete
}: {
  tool_call: ToolCall;
  records: ChatMessageRecord[];
  currentIndex: number;
  isComplete?: boolean
}): ReactNode {
  let metric_data = null;
  let query = null;
  for (let j = currentIndex - 1; j >= 0; j--) {
    const previous_record = records[j];
    if (previous_record.message.tool_calls) {
      const query_tool_call = previous_record.message.tool_calls.find(
        (call) => call.function.name === 'execute_clickhouse_query'
      );
      if (query_tool_call) {
        query = JsonParse(query_tool_call.function.arguments)?.query;
        for (let k = j + 1; k < currentIndex; k++) {
          const next_record = records[k];
          if (
            next_record.message.role === 'tool' &&
            next_record.message.tool_call_id === query_tool_call.id &&
            next_record.message?.content
          ) {
            try {
              metric_data = JSON.parse(next_record.message.content);
            } catch (error) {
              console.error('Failed to parse metric data from tool response', error);
            }
            break;
          }
        }
        break;
      }
    }
  }

  if (metric_data) {
    const args = JsonParse(tool_call.function.arguments);
    if(args == null){
      if(!isComplete){
        // show shimmer
        return;
      }
      // show error
      return
    }
    return (
      <InlineMetric
        metricData={{
          label: args.label,
          query: query ?? '',
          description: args.description,
          chartConfig: args.chart_config,
          response: { response: metric_data },
        }}
      />
    );
  } else {
    return <ToolCallMsg content="Metric is not presentable" isComplete={isComplete} />;
  }
}

function TestRunLiveDashboard({
  tool_call,
  records,
  currentIndex,
  isComplete,
  chatId
}: {
  tool_call: ToolCall;
  records: ChatMessageRecord[];
  currentIndex: number;
  isComplete?: boolean;
  chatId?: string;
}): ReactNode {
  const [isSaving, setIsSaving] = useState(false);
  const [isSaved, setIsSaved] = useState(false);
  const { showSuccess, showError } = useToast();
  const project = useProject();

  const args = JsonParse(tool_call.function.arguments);

  if (args == null) {
    if (!isComplete) {
      return null;
    }
    return <ToolCallMsg content="Invalid dashboard configuration" isComplete={isComplete} />;
  }

  // Find the query tool call by ID
  const queryToolCallId = args.query_tool_call_id;
  let query = null;
  let metric_data = null;

  if (queryToolCallId) {
    // Find the query tool call by ID
    for (let j = currentIndex - 1; j >= 0; j--) {
      const previous_record = records[j];
      if (previous_record.message.tool_calls) {
        const query_tool_call = previous_record.message.tool_calls.find(
          (call) => call.id === queryToolCallId
        );
        if (query_tool_call) {
          query = JsonParse(query_tool_call.function.arguments)?.query;
          
          // Look forward to find the response for this query
          for (let k = j + 1; k < currentIndex; k++) {
            const next_record = records[k];
            if (
              next_record.message.role === 'tool' &&
              next_record.message.tool_call_id === queryToolCallId &&
              next_record.message?.content
            ) {
              try {
                metric_data = JSON.parse(next_record.message.content);
              } catch (error) {
                console.error('Failed to parse metric data from tool response', error);
              }
              break;
            }
          }
          break;
        }
      }
    }
  }

  // Find the message_id containing this tool_call
  let messageId: string | null = null;
  for (let i = currentIndex; i >= 0; i--) {
    const record = records[i];
    if (record.message.tool_calls) {
      const hasToolCall = record.message.tool_calls.some(
        (call) => call.id === tool_call.id
      );
      if (hasToolCall) {
        messageId = record.id;
        break;
      }
    }
  }

  const handleSave = async () => {
    if (!chatId || !messageId) {
      console.error('Missing required info:', { chatId, messageId });
      showError('Missing required information to save dashboard');
      return;
    }

    setIsSaving(true);
    try {
      await dashboardService.saveDashboardFromChat(
        chatId,
        messageId,
        tool_call.id
      );
      setIsSaved(true);
      showSuccess('Dashboard saved successfully');
    } catch (error) {
      console.error('Failed to save dashboard:', error);
      
      if (error instanceof ApiError && error.status === 409) {
        setIsSaved(true);
        showError(error.data?.message || 'This dashboard has already been saved');
      } else {
        showError('Failed to save dashboard');
      }
    } finally {
      setIsSaving(false);
    }
  };

  if (metric_data) {
    // Wrap chart_config in ChartConfigWrapper structure if it's an array
    const chartConfig = Array.isArray(args.chart_config)
      ? { charts: args.chart_config }
      : args.chart_config;

    const saveButton = (
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        {messageId && !isSaved && (
          <Button
            size="small"
            variant="contained"
            color="primary"
            onClick={handleSave}
            disabled={isSaving}
            startIcon={isSaving ? <CircularProgress size={16} /> : <SaveIcon size={16} />}
            sx={{ minWidth: 'auto' }}
          >
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        )}
        {isSaved && (
          <Box sx={{ color: 'success.main', fontSize: '0.875rem', fontWeight: 500 }}>
            Saved!
          </Box>
        )}
      </Box>
    );

    return (
      <InlineMetric
        metricData={{
          label: args.label || '',
          query: query ?? '',
          description: args.description,
          chartConfig: chartConfig,
          response: { response: metric_data },
        }}
        footer={saveButton}
      />
    );
  } else {
    return <ToolCallMsg content="Dashboard preview data is not available" isComplete={isComplete} />;
  }
}


export function renderToolCall(
  tool_call: ToolCall,
  records: ChatMessageRecord[],
  currentIndex: number,
  isComplete?: boolean,
  chatId?: string
): ReactNode {
  switch (tool_call.function.name) {
    case 'request_datetime_range':
      return <RequestDateTimeRange tool_call={tool_call} isComplete={isComplete} />;
    case 'list_golden_queries':
      return <ListGoldenQueries tool_call={tool_call} isComplete={isComplete} />;
    case 'get_current_datetime':
      return <GetCurrentDatetime tool_call={tool_call} isComplete={isComplete} />;
    case 'fetch_golden_query':
      return <FetchGoldenQuery tool_call={tool_call} isComplete={isComplete} />;
    case 'get_database_schema':
      return <GetDatabaseSchema tool_call={tool_call} isComplete={isComplete} />;
    case 'execute_clickhouse_query':
      return <ExecuteClickhouseQuery tool_call={tool_call} isComplete={isComplete} />;
    case 'get_event_descriptions':
      return <GetEventDescriptions tool_call={tool_call} isComplete={isComplete} />;
    case 'present_metric_insight':
      return <PresentMetricInsight tool_call={tool_call} records={records} currentIndex={currentIndex} isComplete={isComplete}/>;
    case 'test_run_live_dashboard':
      return <TestRunLiveDashboard tool_call={tool_call} records={records} currentIndex={currentIndex} isComplete={isComplete} chatId={chatId}/>
    default:
      return null;
  }
}
