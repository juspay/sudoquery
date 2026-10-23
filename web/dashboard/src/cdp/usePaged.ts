import { useState } from 'react';
import { apiErrorMessage } from '../services/apiClient';
import { useAsyncResult } from '../discover/useDiscover';

interface Page<T> {
  items: T[];
  next: string | null;
}

/**
 * The first page for `key` plus any pages loaded after it with "Load more".
 * A new key starts over; extra pages loaded for an older key are dropped.
 */
export function usePaged<P, T>(
  key: string | null,
  load: (cursor: string | null, signal?: AbortSignal) => Promise<P>,
  page: (p: P) => Page<T>,
) {
  const first = useAsyncResult(key, (signal) => load(null, signal));
  const [more, setMore] = useState<{ key: string; items: T[]; next: string | null } | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [moreError, setMoreError] = useState<string | null>(null);

  const firstPage = first.data ? page(first.data) : null;
  const extra = more && more.key === first.dataKey ? more : null;
  const rows = [...(firstPage?.items ?? []), ...(extra?.items ?? [])];
  const nextCursor = extra ? extra.next : (firstPage?.next ?? null);

  const loadMore = async () => {
    const dataKey = first.dataKey;
    if (!nextCursor || !dataKey) return;
    setLoadingMore(true);
    setMoreError(null);
    try {
      const next = page(await load(nextCursor));
      setMore({ key: dataKey, items: [...(extra?.items ?? []), ...next.items], next: next.next });
    } catch (error) {
      setMoreError(apiErrorMessage(error, 'Could not load more'));
    } finally {
      setLoadingMore(false);
    }
  };

  return { first, rows, nextCursor, loadMore, loadingMore, moreError };
}
