import { DEMO_CONFIG } from '../config/api';

/**
 * Where the demo store files its events. The dashboard only shows a project
 * the events whose `org_id` and `proj_id` both match it, so both are needed.
 */
export type Destination = {
  /** Organization ID, sent to the collector as the tenant (`org_id`). */
  tenantId: string;
  /** Project UUID, sent to the collector as the workspace (`proj_id`). */
  workspaceId: string;
};

const STORAGE_KEY = 'sudoquery_demo_destination';

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export const isUuid = (value: string): boolean => UUID_PATTERN.test(value);

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
    if (typeof saved?.tenantId === 'string' && typeof saved.workspaceId === 'string') {
      return { tenantId: saved.tenantId, workspaceId: saved.workspaceId };
    }
  } catch {
    // Unreadable storage or a malformed value; use the environment's instead.
  }
  return null;
}

function fromEnv(): Destination | null {
  const tenantId = DEMO_CONFIG.TENANT_ID;
  const workspaceId = DEMO_CONFIG.WORKSPACE_ID;
  return tenantId && workspaceId ? { tenantId, workspaceId } : null;
}
