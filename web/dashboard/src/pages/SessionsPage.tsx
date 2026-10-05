import { useCallback, useMemo, useState } from 'react';
import { Link as RouterLink, useLocation } from 'react-router-dom';
import { Alert, Box, Button, ButtonBase, CircularProgress, Skeleton, Typography } from '@mui/material';
import { ArrowDown as ArrowDownIcon, ArrowUp as ArrowUpIcon } from 'lucide-react';
import { DiscoverHeader } from '../components/Discover/DiscoverHeader';
import { HAIRLINE, monoSx, overlineSx, panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { EVENT_FIELDS } from '../discover/fields';
import { formatCount, formatDuration } from '../discover/format';
import { formatTimestamp } from '../discover/timeRange';
import { useAsyncResult, useDiscover, useSearchScope } from '../discover/useDiscover';
import {
  isAbort,
  searchErrorMessage,
  searchService,
  type SessionSummary,
} from '../services/searchService';
import { colorBlue, colorCream, colorInk60 } from '../theme/tokens';

const PAGE_SIZE = 100;
const SUGGESTED_VALUES = 10;
const GRID = 'minmax(220px, 2fr) 200px 200px 120px 100px';

type SortColumn = 'session' | 'started' | 'last' | 'duration' | 'events';

const COLUMNS: Array<{ id: SortColumn; label: string; align?: 'right' }> = [
  { id: 'session', label: 'Session' },
  { id: 'started', label: 'Started' },
  { id: 'last', label: 'Last event' },
  { id: 'duration', label: 'Duration', align: 'right' },
  { id: 'events', label: 'Events', align: 'right' },
];

interface Row extends SessionSummary {
  started: number;
  last: number;
  duration: number;
}

function sortValue(row: Row, column: SortColumn): number | string {
  switch (column) {
    case 'session':
      return row.session_id;
    case 'started':
      return row.started;
    case 'last':
      return row.last;
    case 'duration':
      return row.duration;
    case 'events':
      return row.event_count;
  }
}

interface MorePages {
  key: string;
  items: SessionSummary[];
  next: string | null;
}

export default function SessionsPage() {
  const scope = useSearchScope();
  const location = useLocation();
  const discover = useDiscover(EVENT_FIELDS);
  const { built, searchKey } = discover;
  const query = built.ok ? built.query : null;

  const [sort, setSort] = useState<{ column: SortColumn; descending: boolean }>({
    column: 'last',
    descending: true,
  });
  const [more, setMore] = useState<MorePages | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [moreError, setMoreError] = useState<string | null>(null);

  const key = scope && searchKey ? `${scope.projectId}:${searchKey}` : null;
  const first = useAsyncResult(key, (signal) =>
    searchService.listSessions({ ...scope!, signal }, { query: query!, page_size: PAGE_SIZE }),
  );

  const extra = more && more.key === first.dataKey ? more : null;
  const nextCursor = extra ? extra.next : (first.data?.next_cursor ?? null);

  const rows = useMemo(() => {
    const sessions = first.data ? [...first.data.items, ...(extra?.items ?? [])] : [];
    const withTimes = sessions.map((session): Row => {
      const started = new Date(session.first_event_at).getTime();
      const last = new Date(session.last_event_at).getTime();
      return { ...session, started, last, duration: last - started };
    });
    const direction = sort.descending ? -1 : 1;
    return withTimes.sort((a, b) => {
      const left = sortValue(a, sort.column);
      const right = sortValue(b, sort.column);
      return (left < right ? -1 : left > right ? 1 : 0) * direction;
    });
  }, [first.data, extra, sort]);

  const loadMore = async () => {
    if (!scope || !query || !nextCursor || !first.dataKey) return;
    const dataKey = first.dataKey;
    setLoadingMore(true);
    setMoreError(null);
    try {
      const page = await searchService.listSessions(scope, {
        query,
        page_size: PAGE_SIZE,
        cursor: nextCursor,
      });
      setMore({ key: dataKey, items: [...(extra?.items ?? []), ...page.items], next: page.next_cursor });
    } catch (error) {
      if (!isAbort(error)) setMoreError(searchErrorMessage(error, 'Could not load more sessions'));
    } finally {
      setLoadingMore(false);
    }
  };

  const lookupValues = useCallback(
    async (field: string, prefix: string): Promise<string[]> => {
      const definition = EVENT_FIELDS.find((candidate) => candidate.name === field);
      if (!definition?.facetable || !scope || !query) return [];
      const facets = await searchService.facets(scope, {
        query,
        field,
        size: SUGGESTED_VALUES,
        prefix: definition.type === 'boolean' ? undefined : prefix,
      });
      return facets.buckets.map((bucket) => bucket.value);
    },
    [scope, query],
  );

  if (!scope) {
    return (
      <EmptyStateCard
        heading="No project selected"
        subtext="Choose an organization and a project to see its sessions."
      />
    );
  }

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <DiscoverHeader
        tab="sessions"
        discover={discover}
        fields={EVENT_FIELDS}
        loading={first.loading}
        lookupValues={lookupValues}
      />

      <Box sx={{ ...panelSx, flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
        {first.error ? (
          <Alert severity="error" sx={{ m: 1.5 }}>
            {first.error}
          </Alert>
        ) : null}

        {!first.data && first.loading ? (
          <Box sx={{ p: 2 }}>
            {[0, 1, 2, 3, 4, 5].map((row) => (
              <Skeleton key={row} height={34} />
            ))}
          </Box>
        ) : null}

        {first.data && rows.length === 0 && !first.loading ? (
          <EmptyStateCard
            heading="No sessions in this time range"
            subtext="Sessions are built from events that carry a session id. Try a wider time range or fewer filters."
          />
        ) : null}

        {rows.length > 0 ? (
          <>
            <Box sx={{ flex: 1, minHeight: 0, overflow: 'auto', opacity: first.loading ? 0.55 : 1 }}>
              <Box
                sx={{
                  display: 'grid',
                  gridTemplateColumns: GRID,
                  columnGap: 2,
                  px: 2,
                  position: 'sticky',
                  top: 0,
                  zIndex: 1,
                  bgcolor: '#FFFFFF',
                  borderBottom: HAIRLINE,
                }}
              >
                {COLUMNS.map((column) => {
                  const active = sort.column === column.id;
                  const SortIcon = sort.descending ? ArrowDownIcon : ArrowUpIcon;
                  return (
                    <ButtonBase
                      key={column.id}
                      onClick={() =>
                        setSort((current) => ({
                          column: column.id,
                          descending: current.column === column.id ? !current.descending : true,
                        }))
                      }
                      sx={{
                        ...overlineSx,
                        py: 1,
                        gap: 0.5,
                        justifyContent: column.align === 'right' ? 'flex-end' : 'flex-start',
                        color: active ? 'text.primary' : 'text.secondary',
                      }}
                    >
                      {column.label}
                      {active ? <SortIcon size={12} /> : null}
                    </ButtonBase>
                  );
                })}
              </Box>

              {rows.map((row) => (
                <Box
                  key={row.session_id}
                  component={RouterLink}
                  to={{
                    pathname: `/app/discover/sessions/${encodeURIComponent(row.session_id)}`,
                    search: location.search,
                  }}
                  sx={{
                    display: 'grid',
                    gridTemplateColumns: GRID,
                    columnGap: 2,
                    alignItems: 'center',
                    px: 2,
                    py: 0.875,
                    borderBottom: HAIRLINE,
                    textDecoration: 'none',
                    color: 'text.primary',
                    '&:hover': { bgcolor: colorCream },
                  }}
                >
                  <Box
                    sx={{
                      ...monoSx,
                      color: colorBlue,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                    title={row.session_id}
                  >
                    {row.session_id}
                  </Box>
                  <Box sx={monoSx}>{formatTimestamp(new Date(row.started))}</Box>
                  <Box sx={monoSx}>{formatTimestamp(new Date(row.last))}</Box>
                  <Box sx={{ ...monoSx, textAlign: 'right' }}>{formatDuration(row.duration)}</Box>
                  <Box sx={{ ...monoSx, textAlign: 'right' }}>{formatCount(row.event_count)}</Box>
                </Box>
              ))}

              {nextCursor ? (
                <Box sx={{ display: 'flex', justifyContent: 'center', py: 1.5 }}>
                  <Button size="small" variant="outlined" onClick={loadMore} disabled={loadingMore}>
                    {loadingMore ? <CircularProgress size={14} sx={{ mr: 1 }} /> : null}
                    Load more
                  </Button>
                </Box>
              ) : null}
              {moreError ? (
                <Alert severity="error" sx={{ m: 1.5 }}>
                  {moreError}
                </Alert>
              ) : null}
            </Box>

            <Box sx={{ px: 2, py: 0.75, borderTop: HAIRLINE }}>
              <Typography sx={{ fontSize: '12px', color: colorInk60 }}>
                {formatCount(rows.length)} session{rows.length === 1 ? '' : 's'} loaded
                {nextCursor
                  ? ' — more are available. Sorting applies to the loaded sessions; the server lists them by session id.'
                  : ' — all sessions in this range.'}{' '}
                Counts and times cover the events inside the time range.
              </Typography>
            </Box>
          </>
        ) : null}
      </Box>
    </Box>
  );
}
