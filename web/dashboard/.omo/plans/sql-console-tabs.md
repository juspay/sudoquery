# SQL Console Tabs with Per-Console State

## Goal
Fetch consoles from `/api/project/consoles`, display them as horizontal tabs, allow creating new consoles, and wrap the existing editor so each console maintains its own query state.

## Context

### Current State
- **File**: `src/pages/SQLConsole.tsx` (176 lines)
- **Editor**: CodeMirror with SQL syntax highlighting, light theme, line numbers
- **Results**: ag-grid with Google Sheets-styled `themeBalham`
- **Data flow**: Local `useState` for query — no persistence

### API Patterns (from codebase)
- **Client**: `apiClient` singleton in `src/services/apiClient.ts`
  - Auto-injects `Authorization: Bearer <token>`
  - Auto-injects `X-Project-Id` from localStorage
  - Methods: `.get()`, `.post()`, `.put()`, `.delete()`
- **Services**: Each domain has a `*Service.ts` file exporting functions
- **Types**: Interfaces exported from `src/types/api.ts`

### UI Patterns
- **Tabs**: MUI `Tabs` + `Tab` used in `InvitationsSettingsPage.tsx`
  - Horizontal, `textTransform: 'none'`, border-bottom separator
- **Empty state**: Show message + action button (seen in invitations page)
- **Loading**: `CircularProgress` centered

## Type Definition

```typescript
// src/types/api.ts
export interface UserProjectConsole {
  id: string;        // UUID as string
  user_id: string;   // UUID as string
  proj_id: string;   // UUID as string
  console: string | null;  // Stored query content
  name: string | null;     // Console display name
  created_at: string;      // ISO datetime
  updated_at: string;      // ISO datetime
}
```

## Service Layer

```typescript
// src/services/consoleService.ts
import { apiClient } from './apiClient';
import type { UserProjectConsole } from '../types/api';

export const consoleService = {
  async list(): Promise<UserProjectConsole[]> {
    return apiClient.get('/api/project/consoles');
  },

  async create(name: string, console: string): Promise<UserProjectConsole> {
    return apiClient.post('/api/project/consoles', { name, console });
  },

  async update(id: string, name: string, console: string): Promise<UserProjectConsole> {
    return apiClient.put(`/api/project/consoles/${id}`, { name, console });
  },

  async delete(id: string): Promise<void> {
    await apiClient.delete(`/api/project/consoles/${id}`);
  },
};
```

## Component Architecture

```
SQLConsole (page container)
├── ConsoleTabs (tab bar + create button)
│   ├── Tabs (MUI)
│   │   └── Tab[] — one per console
│   └── "+ New Console" button (last tab or adjacent)
└── ConsoleEditor (extracted from current SQLConsole)
    ├── CodeMirror editor (per-console query state)
    ├── Run button
    └── Results grid (ag-grid)
```

### ConsoleEditor Props

```typescript
interface ConsoleEditorProps {
  consoleId: string;
  initialQuery: string;
  onQueryChange: (consoleId: string, query: string) => void;
  onRun?: () => void;  // Optional callback after successful run
}
```

### State Management in SQLConsole

```typescript
// Top-level state
const [consoles, setConsoles] = useState<UserProjectConsole[]>([]);
const [activeConsoleId, setActiveConsoleId] = useState<string>('');
const [queryByConsole, setQueryByConsole] = useState<Record<string, string>>({});
const [loading, setLoading] = useState(true);
const [creating, setCreating] = useState(false);
```

**State flow:**
1. On mount: Fetch consoles → populate list
2. If consoles exist: Set first as active, initialize query state from `console.console`
3. If no consoles: Show "Create Console" empty state
4. On tab switch: `activeConsoleId` changes, editor shows that console's query
5. On query edit: Update `queryByConsole[consoleId]` locally
6. On save/run: Persist to backend via `consoleService.update()`
7. On create: Open modal/dialog → POST → add to list → select it

## Empty State

When `consoles.length === 0`:
```
┌─────────────────────────────┐
│  No consoles yet            │
│                             │
│  [+ Create New Console]     │
└─────────────────────────────┘
```

## Create Console Flow

1. User clicks "+ New Console"
2. Dialog opens with:
   - Name input (required)
   - Optional initial query (CodeMirror)
3. Submit → `consoleService.create(name, query)`
4. On success:
   - Append to `consoles` list
   - Set as active
   - Initialize `queryByConsole[newId] = query`

## UI Layout

```
┌──────────────────────────────────────────┐
│  Untitled-1  Untitled-2  + New Console   │  ← Tabs, scrollable if many
├──────────────────────────────────────────┤
│                                          │
│  ┌────────────────────────────────────┐  │
│  │ SELECT * FROM events LIMIT 100;    │  │  ← ConsoleEditor (CodeMirror)
│  └────────────────────────────────────┘  │
│  [Run]                                   │
│                                          │
│  ┌────────────────────────────────────┐  │
│  │ Rows: 100    Elapsed: 0.042s       │  │
│  ├────────────────────────────────────┤  │
│  │ event_id | timestamp | user_id     │  │  ← Results grid
│  │ ...                                │  │
│  └────────────────────────────────────┘  │
│                                          │
└──────────────────────────────────────────┘
```

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Extract `ConsoleEditor` into separate component | Keeps page container focused on tab/state management; enables reusability |
| Store unsaved query in local `queryByConsole` record | Allows editing without auto-saving; persist only on explicit save or run |
| Use MUI Tabs (not custom) | Already used in codebase; consistent with InvitationsSettingsPage pattern |
| Console name in tab + inline rename | Simple UX; can add dedicated rename later if needed |
| `console` field stores full query string | Backend stores it; frontend loads it as initial state |
| UUIDs as `string` in TypeScript | Matches existing patterns (Project.id, etc. are strings) |

## File Changes

1. **NEW** `src/services/consoleService.ts` — API wrapper
2. **NEW** `src/components/ConsoleEditor.tsx` — Extracted editor + results
3. **MODIFY** `src/types/api.ts` — Add `UserProjectConsole` interface
4. **MODIFY** `src/pages/SQLConsole.tsx` — Refactor to tab container

## Acceptance Criteria

- [x] `/api/project/consoles` is called on page mount
- [x] Consoles render as horizontal tabs with names
- [x] Active tab switches editor/query context
- [x] Empty state shows "Create Console" prompt
- [x] New console creation works with name + optional initial query
- [x] Each console maintains independent query state
- [x] Existing editor functionality (syntax highlighting, run, results) preserved
- [x] No TypeScript errors
- [x] Follows existing service/type/component patterns

## Edge Cases

- **API returns 404** → Show empty state (no consoles yet)
- **Network error** → Show error alert, retry button
- **Tab overflow** → Tabs become horizontally scrollable
- **Delete active console** → Switch to first remaining or show empty state
- **Create without name** → Validate, show error
- **Unsaved changes on tab switch** → Keep in local state (don't warn for now)
