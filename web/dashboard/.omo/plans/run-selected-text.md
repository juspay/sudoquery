# Feature: Run Selected Text in Console

## Requirement
In the SQL console editor, when text is selected, the Run button should:
1. Execute only the selected text (not the entire query)
2. Change button text from "Run" to "Run Selected" when text is selected

## Implementation

### File: `src/components/ConsoleEditor.tsx`

#### 1. Add imports
```typescript
import { useRef } from 'react';  // already imported, just add useRef if not present
import type { ReactCodeMirrorRef } from '@uiw/react-codemirror';
```

#### 2. Add state and ref
```typescript
const editorRef = useRef<ReactCodeMirrorRef>(null);
const [hasSelection, setHasSelection] = useState(false);
```

#### 3. Add selection checker
```typescript
const checkSelection = useCallback(() => {
  const view = editorRef.current?.view;
  if (!view) {
    setHasSelection(false);
    return;
  }
  const selection = view.state.selection.main;
  setHasSelection(selection.from !== selection.to);
}, []);
```

#### 4. Listen for selection changes
```typescript
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
```

#### 5. Modify handleRun to use selected text
```typescript
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
```

#### 6. Update button
```tsx
<Button
  size="small"
  startIcon={loading ? <CircularProgress size={14} color="inherit" /> : <PlayIcon size={14} />}
  onClick={handleRun}
  disabled={loading || !query.trim()}
>
  {hasSelection ? 'Run Selected' : 'Run'}
</Button>
```

#### 7. Add ref to CodeMirror
```tsx
<CodeMirror
  ref={editorRef}
  value={query}
  // ... rest of props
/>
```

## Verification
- Select text in editor → button text changes to "Run Selected"
- Click Run Selected → only selected text executes
- Deselect text → button text returns to "Run"
- No selection → runs entire query as before
- Build passes
