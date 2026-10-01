import { useCallback, useEffect, useEffectEvent, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useOrganization } from '../contexts/OrganizationContext';
import { useProject } from '../contexts/ProjectContext';
import { isAbort, searchErrorMessage, type SearchContext } from '../services/searchService';
import { buildQuery, type BuiltQuery } from './buildQuery';
import { resolveRange } from './timeRange';
import { parseState, serializeState } from './urlState';
import type { DiscoverState, FieldDef, ResolvedRange } from './types';

/** The organization and project searches run against, once both are chosen. */
export function useSearchScope(): SearchContext | null {
  const { currentOrganization } = useOrganization();
  const { currentProject } = useProject();
  const organizationId = currentOrganization?.id;
  const projectId = currentProject?.id;
  return useMemo(
    () => (organizationId && projectId ? { organizationId, projectId } : null),
    [organizationId, projectId],
  );
}

export interface Discover {
  state: DiscoverState;
  /** Changes part of the search; the page URL follows. */
  update: (patch: Partial<DiscoverState>) => void;
  /** Runs the same search again; a relative time range moves to the present. */
  refresh: () => void;
  /** `null` when the time range cannot be read or is empty. */
  range: ResolvedRange | null;
  built: BuiltQuery;
  /** Identifies one run of the search: changes with the query or a refresh. */
  searchKey: string | null;
}

/**
 * The search state, kept in the page URL, and the query it compiles to.
 *
 * "now" is taken when the user changes the search or refreshes, not on every
 * render, so a relative range stays put while its result is on screen.
 */
export function useDiscover(fields: FieldDef[]): Discover {
  const [params, setParams] = useSearchParams();
  const [now, setNow] = useState(() => Date.now());

  const state = useMemo(() => parseState(params), [params]);

  const update = useCallback(
    (patch: Partial<DiscoverState>) => {
      setNow(Date.now());
      setParams((current) => serializeState({ ...parseState(current), ...patch }));
    },
    [setParams],
  );
  const refresh = useCallback(() => setNow(Date.now()), []);

  const { from, to } = state.time;
  const range = useMemo(() => resolveRange({ from, to }, new Date(now)), [from, to, now]);

  const built = useMemo<BuiltQuery>(() => {
    if (!range) {
      return { ok: false, error: 'The time range is not valid: its start must be before its end.' };
    }
    return buildQuery(state, range, fields);
  }, [state, range, fields]);

  const searchKey = useMemo(
    () => (built.ok ? `${now}:${JSON.stringify(built.query)}` : null),
    [built, now],
  );

  return { state, update, refresh, range, built, searchKey };
}

export interface AsyncResult<T> {
  /** The latest loaded value; kept while the next one loads. */
  data: T | undefined;
  /** The key `data` was loaded for. */
  dataKey: string | null;
  error: string | null;
  loading: boolean;
}

/**
 * Loads a value for `key`, reloading when the key changes and cancelling the
 * request it replaces. A `null` key loads nothing.
 */
export function useAsyncResult<T>(
  key: string | null,
  load: (signal: AbortSignal) => Promise<T>,
): AsyncResult<T> {
  const [loaded, setLoaded] = useState<{ key: string; data: T } | null>(null);
  const [failed, setFailed] = useState<{ key: string; error: string } | null>(null);
  const run = useEffectEvent(load);

  useEffect(() => {
    if (key === null) return;
    const controller = new AbortController();
    run(controller.signal).then(
      (data) => setLoaded({ key, data }),
      (error: unknown) => {
        if (!isAbort(error)) {
          setFailed({ key, error: searchErrorMessage(error, 'The request failed') });
        }
      },
    );
    return () => controller.abort();
  }, [key]);

  const error = key !== null && failed?.key === key ? failed.error : null;
  return {
    data: loaded?.data,
    dataKey: loaded?.key ?? null,
    error,
    loading: key !== null && loaded?.key !== key && error === null,
  };
}
