# Fix Console Data Fetch Issues

## Overview
Two related bugs prevent console query content from loading when switching tabs or on initial page load.

## Bug 1: Wrong API Route in `consoleService.get()`

**Location:** `src/services/consoleService.ts:10`

**Current (broken):**
```typescript
async get(consoleId: string): Promise<UserProjectConsole> {
  return apiClient.get(`/project/consoles?console_id=${consoleId}`);
}
```

**Backend routes** (from `edge-server/src/main.rs:409-413`):
```rust
.route("/project/consoles", get(list_consoles))   // returns sparse list: {id, name, created_at, updated_at}
.route("/project/console", get(get_console))      // returns full data: {id, name, console, user_id, proj_id, created_at, updated_at}
```

**Problem:** The frontend calls `/project/consoles?console_id=X` which hits the **list** route. That route ignores the `console_id` query param and returns the sparse list format without the `console` field.

**Fix:** Change endpoint to singular `/project/console`:
```typescript
async get(consoleId: string): Promise<UserProjectConsole> {
  return apiClient.get(`/project/console?console_id=${consoleId}`);
}
```

## Bug 2: Initial Console Content Not Fetched on Page Load

**Location:** `src/pages/SQLConsole.tsx`

**Current flow (broken):**
1. `fetchConsoles()` loads console list → sets `activeConsoleId` to first console
2. Nothing fetches the **content** of that first console
3. Editor displays empty query
4. `handleTabChange` only runs on user click, not on programmatic state change

**Fix:** Add a `useEffect` that watches `activeConsoleId` and fetches content when it changes to an uncached console.

**Insert after line 47** (after the existing `fetchConsoles` useEffect):
```typescript
  useEffect(() => {
    if (!activeConsoleId || activeConsoleId in queryByConsole) return;
    setLoadingConsoleId(activeConsoleId);
    consoleService.get(activeConsoleId)
      .then((console) => {
        setQueryByConsole((prev) => ({ ...prev, [activeConsoleId]: console.console ?? '' }));
      })
      .catch(() => {
        setLoadError('Failed to load console content');
      })
      .finally(() => {
        setLoadingConsoleId(null);
      });
  }, [activeConsoleId]);
```

## Verification Steps

1. Run `npm run build` (or `pnpm build`) to verify clean compilation
2. On page load with existing consoles, verify the first tab's query content loads
3. Switch between tabs, verify content loads on demand
4. Create a new console, verify empty state works

## Files Changed
- `src/services/consoleService.ts` — 1 line (endpoint path)
- `src/pages/SQLConsole.tsx` — add ~14 lines (new useEffect)

## Acceptance Criteria
- [x] `consoleService.get()` calls `/project/console?console_id=...` (singular)
- [x] On initial page load, first console's query content is fetched and displayed
- [x] On tab switch, console content is fetched if not already cached
- [x] No duplicate fetches for already-cached consoles
- [x] Build passes cleanly
