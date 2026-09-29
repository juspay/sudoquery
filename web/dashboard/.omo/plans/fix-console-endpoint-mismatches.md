# Fix Console Endpoint Mismatches (Update/Delete)

## Problem
The frontend calls wrong HTTP methods and paths for update and delete operations, causing 404/405 errors.

### Backend Routes (from edge-server/src/main.rs)
```rust
.route("/project/consoles", post(create_console))
.route("/project/consoles", get(list_consoles))
.route("/project/console", get(get_console))
.route("/project/console", patch(update_console))    // ← PATCH not PUT
.route("/project/console", delete(delete_console))    // ← singular path
```

### Frontend Currently Sends (BROKEN)
```typescript
// UPDATE - WRONG
apiClient.put(`/project/consoles/${id}`, { name, console })
// Sends: PUT /project/consoles/019ea79b-... with body {name, console}

// DELETE - WRONG
apiClient.delete(`/project/consoles/${id}`)
// Sends: DELETE /project/consoles/019ea79b-...
```

### Backend Expects
```typescript
// UPDATE
PATCH /project/console
body: { console_id: id, name, console }

// DELETE
DELETE /project/console?console_id={id}
```

## Changes Required

### File: `src/services/consoleService.ts`

**Change update() method:**
```typescript
async update(id: string, name: string, console: string): Promise<UserProjectConsole> {
  return apiClient.patch('/project/console', { console_id: id, name, console });
}
```

**Change delete() method:**
```typescript
async delete(id: string): Promise<void> {
  await apiClient.delete('/project/console', { params: { console_id: id } });
}
```

## Verification
1. Build passes: `pnpm build`
2. LSP diagnostics clean on `src/services/consoleService.ts`
3. Test update: modify console name → should succeed with 200
4. Test delete: delete console → should succeed with 204

## Acceptance Criteria
- [ ] `consoleService.update()` sends PATCH to `/project/console` with `{console_id, name, console}`
- [ ] `consoleService.delete()` sends DELETE to `/project/console?console_id={id}`
- [ ] Build passes cleanly
- [ ] No TypeScript errors
