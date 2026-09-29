```typescript
import { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import {
  Box,
  Button,
  Paper,
  Typography,
  Alert,
  CircularProgress,
} from '@mui/material';
import { Play as PlayIcon } from 'lucide-react';
import { AgGridReact } from 'ag-grid-react';
import { AllCommunityModule, themeBalham } from 'ag-grid-community';
import CodeMirror from '@uiw/react-codemirror';
import type { ReactCodeMirrorRef } from '@uiw/react-codemirror';
import { sql } from '@codemirror/lang-sql';
import 'ag-grid-community/styles/ag-grid.css';
import { executeQuery } from '../services/clickhouseService';
import { consoleService } from '../services/consoleService';
import type { QueryResponseData } from '../types/api';

const googleSheetsTheme = themeBalham.withParams({
  accentColor: '#1a73e8',
  borderColor: '#dadce0',
  spacing: 4,
  borderRadius: 2,
  fontSize: 13,
  headerFontWeight: 500,
  oddRowBackgroundColor: { ref: 'backgroundColor', mix: 0.02 },
  headerTextColor: '#5f6368',
  iconButtonHoverBackgroundColor: 'rgba(60,64,67,0.04)',
  iconButtonActiveBackgroundColor: 'rgba(60,64,67,0.08)',
  checkboxBorderRadius: 2,
  checkboxBorderWidth: 1.5,
  checkboxUncheckedBorderColor: '#bdc1c6',
});

const modules = [AllCommunityModule];

interface ConsoleEditorProps {
  consoleId: string;
  consoleName: string;
  initialQuery: string;
  onQueryChange?: (query: string) => void;
}

export default function ConsoleEditor({ consoleId, consoleName, initialQuery, onQueryChange }: ConsoleEditorProps) {
  const [query, setQuery] = useState(initialQuery);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<QueryResponseData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const [hasSelection, setHasSelection] = useState(false);

  // Resize state
  const [editorHeight, setEditorHeight] = useState(150);
  const isDragging = useRef(false);
  const startY = useRef(0);
  const startHeight = useRef(0);

  useEffect(() => {
    setQuery(initialQuery);
  }, [initialQuery]);

  const handleRun = useCallback(async () => {
    const view = editorRef.current?.view;
    let queryToRun = query.trim();

    if (view) {
      const selection = view.state.selection.main;
      if (selection.from !== selection.to) {
        queryToRun = view.state.doc.sliceString(selection.from, selection.to).trim();
      }
    }

    if (!queryToRun) return;
    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const res = await executeQuery(queryToRun);
      if (!res.success) {
        setError(res.error || 'Query failed');
        return;
      }
      if (res.response) {
        setResult(res.response);
      }
      setSaving(true);
      try {
        await consoleService.update(consoleId, consoleName, query);
      } catch {
      } finally {
        setSaving(false);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  }, [query, consoleId, consoleName]);

  const handleQueryChange = useCallback((value: string) => {
    setQuery(value);
    onQueryChange?.(value);
  }, [onQueryChange]);

  const checkSelection = useCallback(() => {
    const view = editorRef.current?.view;
    if (!view) {
      setHasSelection(false);
      return;
    }
    const selection = view.state.selection.main;
    setHasSelection(selection.from !== selection.to);
  }, []);

  useEffect(() => {
    const editorEl = editorRef.current?.editor;
    if (!editorEl) return;

    const events = ['mouseup', 'keyup', 'mousedown'];
    events.forEach((event) => {
      editorEl.addEventListener(event, checkSelection);
    });

    return () => {
      events.forEach((event) => {
        editorEl.removeEventListener(event, checkSelection);
      });
    };
  }, [checkSelection]);

  // Drag handlers for resizable splitter
  const handleSplitterMouseDown = useCallback((e: React.MouseEvent) => {
    isDragging.current = true;
    startY.current = e.clientY;
    startHeight.current = editorHeight;
    document.body.style.cursor = 'row-resize';
    document.body.style.userSelect = 'none';
  }, [editorHeight]);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDragging.current) return;
      const delta = e.clientY - startY.current;
      setEditorHeight(Math.max(80, Math.min(startHeight.current + delta, 400)));
    };
    const handleMouseUp = () => {
      isDragging.current = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, []);

  const colDefs = useMemo(() => {
    if (!result?.meta) return [];
    return result.meta.map((col) => ({
      field: col.name,
      headerName: col.name,
      sortable: true,
      filter: true,
      resizable: true,
    }));
  }, [result]);

  const rowData = useMemo(() => result?.data ?? [], [result]);

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* Editor */}
      <Paper elevation={0} sx={{ border: '1px solid', borderColor: 'divider', borderRadius: 2, overflow: 'hidden', flexShrink: 0 }}>
        <Box sx={{ p: 0, display: 'flex', flexDirection: 'column' }}>
          <Box
            sx={{
              '& .cm-editor': {
                border: 'none',
                outline: 'none',
              },
              '& .cm-focused': {
                outline: 'none',
              },
              '& .cm-gutters': {
                backgroundColor: 'transparent',
                border: 'none',
              },
              '& .cm-activeLineGutter': {
                backgroundColor: 'transparent',
              },
            }}
          >
            <CodeMirror
              ref={editorRef}
              value={query}
              height={`${editorHeight}px`}
              extensions={[sql()]}
              theme="light"
              onChange={handleQueryChange}
              placeholder="Enter your SQL query..."
              basicSetup={{
                lineNumbers: true,
                highlightActiveLineGutter: false,
                highlightActiveLine: false,
                foldGutter: true,
              }}
            />
          </Box>
          <Button
            size="small"
            startIcon={loading ? <CircularProgress size={14} color="inherit" /> : <PlayIcon size={14} />}
            onClick={handleRun}
            disabled={loading || !query.trim()}
          >
            {hasSelection ? 'Run Selected' : 'Run'}
          </Button>
        </Box>
      </Paper>

      {/* Draggable Splitter */}
      <Box
        onMouseDown={handleSplitterMouseDown}
        sx={{
          height: 6,
          cursor: 'row-resize',
          bgcolor: 'divider',
          flexShrink: 0,
          transition: 'background-color 0.15s',
          '&:hover': { bgcolor: 'primary.main' },
        }}
      />

      {/* Error */}
      {error && (
        <Alert severity="error" sx={{ borderRadius: 2, mx: 2, mt: 1 }}>
          {error}
        </Alert>
      )}

      {/* Results */}
      {result && (
        <Paper
          elevation={0}
          sx={{
            flex: 1,
            border: '1px solid',
            borderColor: 'divider',
            borderRadius: 2,
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
            minHeight: 100,
          }}
        >
          {/* Stats bar */}
          <Box
            sx={{
              px: 2,
              py: 1,
              borderBottom: '1px solid',
              borderColor: 'divider',
              bgcolor: 'action.hover',
              display: 'flex',
              gap: 2,
              flexShrink: 0,
            }}
          >
            <Typography variant="caption" color="text.secondary">
              Rows: <strong>{result.rows.toLocaleString()}</strong>
            </Typography>
            <Typography variant="caption" color="text.secondary">
              Elapsed: <strong>{(result.statistics.elapsed / 1000).toFixed(3)}s</strong>
            </Typography>
          </Box>

          {/* AG Grid */}
          <Box sx={{ flex: 1, overflow: 'hidden' }} className="sql-results-grid">
            <AgGridReact
              modules={modules}
              theme={googleSheetsTheme}
              columnDefs={colDefs}
              rowData={rowData}
              defaultColDef={{
                sortable: true,
                filter: true,
                resizable: true,
                minWidth: 80,
              }}
              domLayout="normal"
              animateRows={false}
              rowHeight={28}
              headerHeight={32}
            />
          </Box>
        </Paper>
      )}
    </Box>
  );
}
```

## How to Apply

1. Copy the code above
2. Replace the entire contents of `src/components/ConsoleEditor.tsx`
3. Run `pnpm build` to verify

## What Changed

### Added drag-to-resize functionality:
- **Resize state**: `editorHeight` (default 150px, min 80px, max 400px)
- **Drag handlers**: Track mouse position and adjust height
- **Draggable splitter**: A 6px bar between editor and results that shows `row-resize` cursor on hover
- **Dynamic CodeMirror height**: Uses `height={editorHeight}px` instead of fixed `"150px"`

### Layout adjustments:
- Removed `gap: 2` from outer Box (splitter handles spacing)
- Added `flexShrink: 0` to editor Paper so it doesn't shrink
- Added `minHeight: 100` to results Paper so it stays visible

## Behavior
- Drag the splitter up/down to adjust editor height
- Results panel automatically shrinks/grows
- Editor constrained between 80px and 400px
- Splitter highlights on hover