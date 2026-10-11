/**
 * localStorage keys for the currently selected organization and project.
 *
 * Suffixed `_v2`: the v1 keys stored UUIDs, but ids are now server-generated
 * slugs — a stale UUID never matches, so old sessions fall back to the first
 * organization/project instead of restoring a dead selection.
 *
 * Single source of truth: import these instead of inlining the literal key
 * strings, so every reader/writer stays in sync.
 */
export const ORG_STORAGE_KEY = 'current_organization_id_v2';
export const PROJECT_STORAGE_KEY = 'current_project_id_v2';
