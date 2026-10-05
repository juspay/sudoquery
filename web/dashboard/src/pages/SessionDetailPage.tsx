import { useMemo, useState } from 'react';
import { Link as RouterLink, useLocation, useParams } from 'react-router-dom';
import { Alert, Box, Button, CircularProgress, Skeleton, Typography } from '@mui/material';
import { ArrowLeft as BackIcon, ExternalLink as OpenIcon } from 'lucide-react';
import { QueryBar } from '../components/Discover/QueryBar';
import { monoSx, overlineSx, panelSx } from '../components/Discover/styles';
import { SessionStrip, type SessionEvent } from '../components/Sessions/SessionStrip';
import { SessionTimeline } from '../components/Sessions/SessionTimeline';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { buildQuery } from '../discover/buildQuery';
import { EVENT_FIELDS, TIME_FIELD, discoverFields, flattenEvent } from '../discover/fields';
import { eventElementId, formatCount, formatDuration, nameColors } from '../discover/format';
import { formatTimestamp } from '../discover/timeRange';
import { topValues } from '../discover/topValues';
import type { EventDoc, FilterValue } from '../discover/types';
import { DEFAULT_STATE, serializeState } from '../discover/urlState';
import { useAsyncResult, useSearchScope } from '../discover/useDiscover';
import {
  isAbort,
  searchErrorMessage,
  searchService,
  type SearchContext,
} from '../services/searchService';
import { colorBlue, colorInk40, colorInk60 } from '../theme/tokens';

const PAGE_SIZE = 100;
/** Events fetched up front; longer sessions continue with "Load more". */
const AUTOLOAD_LIMIT = 1000;
const SUGGESTED_VALUES = 10;
/** Widens the Discover time range around the session, so its edges are included. */
const DISCOVER_MARGIN_MS = 1000;

interface Loaded {
  items: EventDoc[];
  next: string | null;
}

async function loadSession(
  context: SearchContext,
  sessionId: string,
  query: Record<string, unknown>,
  cursor: string | null,
  limit: number,
): Promise<Loaded> {
  const items: EventDoc[] = [];
  let next = cursor;
  do {
    const page = await searchService.sessionEvents(context, sessionId, {
      query,
      page_size: PAGE_SIZE,
      cursor: next ?? undefined,
    });
    items.push(...page.items);
    next = page.next_cursor;
  } while (next && items.length < limit);
  return { items, next };
}

function eventTime(doc: EventDoc): number {
  const raw = doc[TIME_FIELD];
  return typeof raw === 'string' ? new Date(raw).getTime() : Number.NaN;
}

/** The distinct values of a field across the session, for the header. */
function distinct(docs: EventDoc[], field: string): string[] {
  const values = new Set<string>();
  for (const doc of docs) {
    const value = flattenEvent(doc)[field];
    if (typeof value === 'string' && value !== '') values.add(value);
  }
  return [...values];
}

function dqlLiteral(value: FilterValue): string {
  return typeof value === 'string'
    ? `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
    : String(value);
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <Box sx={{ minWidth: 0 }}>
      <Typography sx={overlineSx}>{label}</Typography>
      <Box sx={{ ...monoSx, fontSize: '13px', color: 'text.primary', overflowWrap: 'anywhere' }}>
        {children}
      </Box>
    </Box>
  );
}

export default function SessionDetailPage() {
  const { sessionId = '' } = useParams();
  const location = useLocation();
  const scope = useSearchScope();

  const [filter, setFilter] = useState('');
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());
  const [more, setMore] = useState<{ key: string; items: EventDoc[]; next: string | null } | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [moreError, setMoreError] = useState<string | null>(null);

  const built = useMemo(
    () => buildQuery({ query: filter, language: 'dql', filters: [] }, undefined, EVENT_FIELDS),
    [filter],
  );
  const query = built.ok ? built.query : null;
  const key = scope && query ? `${scope.projectId}:${sessionId}:${JSON.stringify(query)}` : null;

  const first = useAsyncResult(key, (signal) =>
    loadSession({ ...scope!, signal }, sessionId, query!, null, AUTOLOAD_LIMIT),
  );
  const extra = more && more.key === first.dataKey ? more : null;
  const nextCursor = extra ? extra.next : (first.data?.next ?? null);

  const events = useMemo(
    () => (first.data ? [...first.data.items, ...(extra?.items ?? [])] : []),
    [first.data, extra],
  );
  const times = useMemo(() => events.map(eventTime), [events]);
  const fields = useMemo(() => discoverFields(events), [events]);
  const strip = useMemo(
    () =>
      events.map(
        (doc, index): SessionEvent => ({
          index,
          name: typeof doc.name === 'string' ? doc.name : '(unnamed)',
          time: times[index],
        }),
      ),
    [events, times],
  );

  const colors = useMemo(() => nameColors(strip.map((event) => event.name)), [strip]);

  const start = times[0];
  const end = times[times.length - 1];

  const applyFilter = (next: string) => {
    setExpanded(new Set());
    setFilter(next);
  };
  const addClause = (clause: string) =>
    applyFilter(filter.trim() === '' ? clause : `${filter.trim()} and ${clause}`);

  const toggle = (index: number) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (!next.delete(index)) next.add(index);
      return next;
    });
  const reveal = (index: number) => {
    setExpanded((current) => new Set(current).add(index));
    requestAnimationFrame(() => {
      document
        .getElementById(eventElementId(index))
        ?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
  };

  const loadMore = async () => {
    if (!scope || !query || !nextCursor || !first.dataKey) return;
    const dataKey = first.dataKey;
    setLoadingMore(true);
    setMoreError(null);
    try {
      const page = await loadSession(scope, sessionId, query, nextCursor, AUTOLOAD_LIMIT);
      setMore({ key: dataKey, items: [...(extra?.items ?? []), ...page.items], next: page.next });
    } catch (error) {
      if (!isAbort(error)) setMoreError(searchErrorMessage(error, 'Could not load more events'));
    } finally {
      setLoadingMore(false);
    }
  };

  const lookupValues = async (field: string, prefix: string): Promise<string[]> =>
    topValues(events, field, 50)
      .buckets.map((bucket) => bucket.label)
      .filter((label) => label.toLowerCase().startsWith(prefix.toLowerCase()))
      .slice(0, SUGGESTED_VALUES);

  const discoverHref = useMemo(() => {
    const params = serializeState({
      ...DEFAULT_STATE,
      filters: [{ field: 'session_id', operator: 'is', value: sessionId, negate: false, disabled: false }],
      ...(Number.isFinite(start) && Number.isFinite(end)
        ? {
            time: {
              from: new Date(start - DISCOVER_MARGIN_MS).toISOString(),
              to: new Date(end + DISCOVER_MARGIN_MS).toISOString(),
            },
          }
        : {}),
    });
    return `/app/discover?${params.toString()}`;
  }, [sessionId, start, end]);

  if (!scope) {
    return (
      <EmptyStateCard
        heading="No project selected"
        subtext="Choose an organization and a project to view a session."
      />
    );
  }

  const filtered = filter.trim() !== '';
  const anonIds = distinct(events, 'anon_id');
  const actorIds = distinct(events, 'actor_id');
  const sources = distinct(events, 'source');
  const countries = distinct(events, 'system_properties.geo.country');

  return (
    <Box sx={{ height: '100%', overflow: 'auto', p: 2, pt: 1 }}>
      <Box sx={{ maxWidth: 1180, mx: 'auto', display: 'flex', flexDirection: 'column', gap: 1.5 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <Button
            component={RouterLink}
            to={{ pathname: '/app/discover/sessions', search: location.search }}
            size="small"
            startIcon={<BackIcon size={14} />}
            sx={{ fontSize: '12px', color: colorInk60 }}
          >
            Sessions
          </Button>
          <Box sx={{ flex: 1 }} />
          <Button
            component={RouterLink}
            to={discoverHref}
            size="small"
            endIcon={<OpenIcon size={13} />}
            sx={{ fontSize: '12px' }}
          >
            View in Discover
          </Button>
        </Box>

        <Box sx={{ ...panelSx, p: 2 }}>
          <Typography sx={overlineSx}>Session</Typography>
          <Typography sx={{ ...monoSx, fontSize: '16px', fontWeight: 600, color: 'text.primary', overflowWrap: 'anywhere' }}>
            {sessionId}
          </Typography>

          {events.length > 0 ? (
            <>
              <Box
                sx={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))',
                  gap: 2,
                  mt: 2,
                }}
              >
                <Fact label="Duration">{formatDuration(end - start)}</Fact>
                <Fact label={filtered ? 'Matching events' : 'Events'}>
                  {formatCount(events.length)}
                  {nextCursor ? '+' : ''}
                </Fact>
                <Fact label="Started">{formatTimestamp(new Date(start))}</Fact>
                <Fact label="Last event">{formatTimestamp(new Date(end))}</Fact>
                {anonIds.length > 0 ? <Fact label="Anonymous id">{anonIds.join(', ')}</Fact> : null}
                {actorIds.length > 0 ? <Fact label="Actor">{actorIds.join(', ')}</Fact> : null}
                {sources.length > 0 ? <Fact label="Source">{sources.join(', ')}</Fact> : null}
                {countries.length > 0 ? <Fact label="Country">{countries.join(', ')}</Fact> : null}
              </Box>
              <Box sx={{ mt: 2 }}>
                <SessionStrip events={strip} colors={colors} onSelect={reveal} />
              </Box>
            </>
          ) : null}
        </Box>

        <QueryBar
          query={filter}
          language="dql"
          fields={fields}
          error={built.ok ? null : built.error}
          placeholder="Filter this session's events — e.g. name:checkout_viewed or properties.plan:pro"
          onSubmit={applyFilter}
          lookupValues={lookupValues}
        />

        {first.error ? <Alert severity="error">{first.error}</Alert> : null}

        {!first.data && first.loading ? (
          <Box sx={{ ...panelSx, p: 2 }}>
            {[0, 1, 2, 3, 4].map((row) => (
              <Skeleton key={row} height={38} />
            ))}
          </Box>
        ) : null}

        {first.data && events.length === 0 && !first.loading ? (
          <Box sx={panelSx}>
            <EmptyStateCard
              heading={filtered ? 'No events in this session match the filter' : 'No events found for this session'}
              subtext={
                filtered
                  ? 'Clear or change the filter to see the session again.'
                  : 'The session id may be wrong, or it belongs to another project.'
              }
              ctaLabel={filtered ? 'Clear filter' : undefined}
              onCta={filtered ? () => applyFilter('') : undefined}
            />
          </Box>
        ) : null}

        {events.length > 0 ? (
          <Box sx={{ opacity: first.loading ? 0.55 : 1 }}>
            <SessionTimeline
              events={events}
              colors={colors}
              times={times}
              expanded={expanded}
              onToggle={toggle}
              columns={[]}
              onFilter={(field, value, negate) =>
                addClause(`${negate ? 'not ' : ''}${field}:${dqlLiteral(value)}`)
              }
              onFilterExists={(field) => addClause(`${field}:*`)}
            />
            {nextCursor ? (
              <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 1.5, py: 1.5 }}>
                <Typography sx={{ fontSize: '12px', color: colorInk40 }}>
                  Showing the first {formatCount(events.length)} events.
                </Typography>
                <Button size="small" variant="outlined" onClick={loadMore} disabled={loadingMore}>
                  {loadingMore ? <CircularProgress size={14} sx={{ mr: 1 }} /> : null}
                  Load more
                </Button>
              </Box>
            ) : (
              <Typography sx={{ py: 1.5, textAlign: 'center', fontSize: '12px', color: colorInk40 }}>
                End of session
                {filtered ? ' (filtered)' : ''} ·{' '}
                <Box component="span" sx={{ color: colorBlue }}>
                  {formatCount(events.length)} event{events.length === 1 ? '' : 's'}
                </Box>
              </Typography>
            )}
            {moreError ? <Alert severity="error">{moreError}</Alert> : null}
          </Box>
        ) : null}
      </Box>
    </Box>
  );
}
