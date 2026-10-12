import { DEMO_CONFIG } from '../config/api';

/**
 * Where the demo store files its events. The collector requires both ids, and
 * the dashboard only shows a project the events whose `org_id` and
 * `project_id` both match it.
 */
export type Destination = {
  /** Organization slug, sent to the collector as `x-org-id` (`org_id`). */
  orgId: string;
  /** Project slug, sent to the collector as `x-project-id` (`project_id`). */
  projectId: string;
};

// Suffixed `_v2`: the v1 key stored a tenant and a workspace UUID, which the
// collector now rejects, so old saved destinations are dropped and the page
// asks again.
const STORAGE_KEY = 'sudoquery_demo_destination_v2';

// The collector's slug rule for `x-org-id` and `x-project-id`.
const SLUG_PATTERN = /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/;

export const isSlug = (value: string): boolean => SLUG_PATTERN.test(value);

/** The destination last picked on the page, else the one set in the environment. */
export function loadDestination(): Destination | null {
  return loadSaved() ?? fromEnv();
}

export function saveDestination(destination: Destination): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(destination));
  } catch {
    // Storage can be unavailable, e.g. with site data blocked. The page then
    // falls back to the environment's destination on the next load.
  }
}

function loadSaved(): Destination | null {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<Destination> | null;
    if (typeof saved?.orgId === 'string' && typeof saved.projectId === 'string') {
      return { orgId: saved.orgId, projectId: saved.projectId };
    }
  } catch {
    // Unreadable storage or a malformed value; use the environment's instead.
  }
  return null;
}

function fromEnv(): Destination | null {
  const orgId = DEMO_CONFIG.ORG_ID;
  const projectId = DEMO_CONFIG.PROJECT_ID;
  return orgId && projectId ? { orgId, projectId } : null;
}
