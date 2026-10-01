import { ApiError, apiClient } from './apiClient';
import type { EventDoc, SortOrder } from '../discover/types';

type Dsl = Record<string, unknown>;

export interface CursorPage<T> {
  items: T[];
  /** `null` on the last page. */
  next_cursor: string | null;
}

export interface SessionSummary {
  session_id: string;
  first_event_at: string;
  last_event_at: string;
  event_count: number;
}

export interface Histogram {
  /** Bucket width, such as `30s` or `1h`. */
  interval: string;
  buckets: Array<{ time: string; count: number }>;
}

export interface Facets {
  field: string;
  /** Events the query matched. */
  total: number;
  /** Events whose value is not among `buckets`. */
  other: number;
  buckets: Array<{ value: string; count: number }>;
}

interface PageRequest {
  query?: Dsl;
  cursor?: string;
  page_size?: number;
  order?: SortOrder;
}

/** Identifies the project a request is for, and lets a stale request be cancelled. */
export interface SearchContext {
  organizationId: string;
  projectId: string;
  signal?: AbortSignal;
}

function options({ organizationId, projectId, signal }: SearchContext) {
  return {
    headers: { 'X-Organization-Id': organizationId, 'X-Project-Id': projectId },
    signal,
  };
}

/** A request cancelled because a newer one replaced it. */
export function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError';
}

/** The server's `{"error": "…"}` message, which `apiClient` leaves as raw JSON text. */
export function searchErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof ApiError) {
    const data = error.data as { error?: unknown } | undefined;
    if (typeof data?.error === 'string') return data.error;
  }
  return error instanceof Error && error.message ? error.message : fallback;
}

export const searchService = {
  async search(context: SearchContext, request: PageRequest): Promise<CursorPage<EventDoc>> {
    return apiClient.post('/search', request, options(context));
  },

  async count(context: SearchContext, query?: Dsl): Promise<number> {
    const response = await apiClient.post<{ count: number }>('/count', { query }, options(context));
    return response.count;
  },

  async getDoc(context: SearchContext, id: string): Promise<EventDoc> {
    return apiClient.get(`/doc/${encodeURIComponent(id)}`, undefined, options(context));
  },

  /** Event counts over `[from, to)`; the server picks the bucket width. */
  async histogram(context: SearchContext, query: Dsl, from: Date, to: Date): Promise<Histogram> {
    return apiClient.post(
      '/histogram',
      { query, from: from.toISOString(), to: to.toISOString() },
      options(context),
    );
  },

  /** Exact top values of an aggregatable field. The query must be bounded in time. */
  async facets(
    context: SearchContext,
    request: { query: Dsl; field: string; size?: number; prefix?: string },
  ): Promise<Facets> {
    return apiClient.post('/facets', request, options(context));
  },

  /** Sessions among the matching events. The query must be bounded in time. */
  async listSessions(
    context: SearchContext,
    request: PageRequest,
  ): Promise<CursorPage<SessionSummary>> {
    return apiClient.post('/session', request, options(context));
  },

  /** The events of one session, oldest first. */
  async sessionEvents(
    context: SearchContext,
    sessionId: string,
    request: PageRequest,
  ): Promise<CursorPage<EventDoc>> {
    return apiClient.post(`/session/${encodeURIComponent(sessionId)}`, request, options(context));
  },
};
