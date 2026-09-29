# Adjustable Console and Result Panel Heights

## Current State
- Console (editor) has a fixed `height="150px"` on the CodeMirror component
- Results panel has `flex: 1` so it fills remaining vertical space
- There's no way for the user to drag/adjust the split between them

## Proposed Solution
Use a resize splitter between the console editor and the results panel:

1. **Track console height** in state (e.g., `editorHeight`)
2. **Render a draggable splitter** between the console and results panels
3. **On drag**: adjust `editorHeight` proportionally, shrinking/growing the results area
4. **Set minimum heights** so neither panel gets too small

## Implementation Details

### Approach: Resizable Split Pane
Wrap both panels in a flex container and add a draggable splitter handle between them.

### State:
```typescript
const [editorHeight, setEditorHeight] = useState(150);
const isDragging = useRef(false);
const startY = useRef(0);
const startHeight = useRef(0);
```

### Handler:
```typescript
const handleMouseDown = (e: React.MouseEvent) => {
  isDragging.current = true;
  startY.current = e.clientY;
  startHeight.current = editorHeight;
  document.body.style.cursor = 'row-resize';
  document.body.style.userSelect = 'none';
};

useEffect(() => {
  const handleMouseMove = (e: MouseEvent) => {
    if (!isDragging.current) return;
    const delta = e.clientY - startY.current;
    setEditorHeight(Math.max(80, startHeight.current + delta));
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
```

### Layout:
```tsx
<Box sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
  {/* Editor — fixed height */}
  <Paper sx={{ height: editorHeight, ... }}>...</Paper>
  
  {/* Draggable Splitter */}
  <Box
    onMouseDown={handleMouseDown}
    sx={{
      height: 4,
      cursor: 'row-resize',
      bgcolor: 'divider',
      '&:hover': { bgcolor: 'primary.main' },
    }}
  />
  
  {/* Results — fills remaining */}
  <Paper sx={{ flex: 1, minHeight: 100, ... }}>...</Paper>
</Box>
```

## Files Changed
- `src/components/ConsoleEditor.tsx`

## Verification
1. Dragging the splitter adjusts console height
2. Results panel shrinks/grows automatically
3. Minimum height enforced (80px console, 100px results)
4. Build passes
