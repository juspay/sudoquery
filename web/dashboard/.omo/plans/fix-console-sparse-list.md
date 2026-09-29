# Fix Frontend Type for Sparse Console List API

## Problem
The backend's `list_consoles_by_user_and_project` only returns `{ id, name, created_at, updated_at }`. It does NOT return `user_id`, `proj_id`, or `console` (the query string). The frontend's `UserProjectConsole` interface incorrectly marks all fields as required, causing runtime issues when accessing `c.console` from the list response.

## Changes Required

### 1. `src/types/api.ts` — Make fields optional

```typescript
export interface UserProjectConsole {
  id: string;
  name: string | null;
  created_at: string;
  updated_at: string;
  user_id?: string;       // Only present on create/get
  proj_id?: string;       // Only present on create/get
  console?: string | null; // Only present on create/get, NOT on list
}
```

### 2. `src/services/consoleService.ts` — Add `get` method

```typescript
async get(consoleId: string): Promise<UserProjectConsole> {
  return apiClient.get(`/api/project/consoles?console_id=${consoleId}`);
}
```

### 3. `src/pages/SQLConsole.tsx` — Fetch console content per tab

**Current broken logic:**
```typescript
// This fails because c.console is undefined from list API
initialQueries[c.id] = c.console ?? '';
```

**Fix:**
- Remove the `initialQueries` population from list response
- Add state: `loadingConsoleId: string | null` to track which console's content is being fetched
- When tab changes (`handleTabChange`):
  - If `queryByConsole[consoleId]` already exists → use cached
  - Else → call `consoleService.get(consoleId)`, then:
    - Set `queryByConsole[consoleId] = fetchedConsole.console ?? ''`
- Show a small loading indicator in the editor area while fetching

**State changes:**
```typescript
const [queryByConsole, setQueryByConsole] = useState<Record<string, string>>({});
const [loadingConsoleId, setLoadingConsoleId] = useState<string | null>(null);
```

**Updated tab change handler:**
```typescript
const handleTabChange = async (_: React.SyntheticEvent, newValue: string) => {
  setActiveConsoleId(newValue);
  
  // Fetch console content if not cached
  if (!(newValue in queryByConsole)) {
    setLoadingConsoleId(newValue);
    try {
      const console = await consoleService.get(newValue);
      setQueryByConsole(prev => ({ ...prev, [newValue]: console.console ?? '' }));
    } catch (err) {
      setLoadError('Failed to load console content');
    } finally {
      setLoadingConsoleId(null);
    }
  }
};
```

**Editor rendering:**
```typescript
{activeConsole && (
  loadingConsoleId === activeConsole.id ? (
    <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
      <CircularProgress size={24} />
    </Box>
  ) : (
    <ConsoleEditor
      key={activeConsole.id}
      initialQuery={queryByConsole[activeConsole.id] ?? ''}
      onQueryChange={(query) => handleQueryChange(activeConsole.id, query)}
    />
  )
)}
```

## Acceptance Criteria
- [x] `UserProjectConsole` type reflects actual API response shape
- [x] `consoleService.get()` added for fetching single console
- [x] Tab switch fetches console content on first visit
- [x] Subsequent tab switches use cached content (no re-fetch)
- [x] Loading state shown while fetching console content
- [x] No TypeScript errors
- [x] Build passes
