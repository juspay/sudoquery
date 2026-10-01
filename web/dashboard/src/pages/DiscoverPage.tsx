import { useCallback, useMemo, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { Alert, Box, Button, CircularProgress, Menu, MenuItem, Skeleton, Typography } from '@mui/material';
import { Download as DownloadIcon } from 'lucide-react';
import { DiscoverHeader } from '../components/Discover/DiscoverHeader';
import { DocTable } from '../components/Discover/DocTable';
import { FieldSidebar } from '../components/Discover/FieldSidebar';
import { Histogram } from '../components/Discover/Histogram';
import { ResizeHandle } from '../components/Discover/ResizeHandle';
import { HAIRLINE, panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { discoverFields, flattenEvent } from '../discover/fields';
import { addExistsFilter, addValueFilter } from '../discover/filters';
import { formatCount } from '../discover/format';
import { formatTimestamp } from '../discover/timeRange';
import { topValues } from '../discover/topValues';
import type { EventDoc, FilterValue } from '../discover/types';
import { useAsyncResult, useDiscover, useSearchScope } from '../discover/useDiscover';
import { usePersistentSize } from '../hooks/usePersistentSize';
import { isAbort, searchErrorMessage, searchService } from '../services/searchService';
import { colorInk40, colorInk60 } from '../theme/tokens';
import { downloadAsCSV, downloadAsJSON } from '../utils/download';

const PAGE_SIZE = 100;
const SUGGESTED_VALUES = 10;
const NO_DOCS: EventDoc[] = [];

/** Pages loaded after the first one, for the search identified by `key`. */
interface MorePages {
  key: string;
  items: EventDoc[];
  next: string | null;
}

/** Flat rows with the same columns on every row, as the CSV export needs. */
function exportRows(docs: EventDoc[], columns: string[]): Record<string, unknown>[] {
  const flat = docs.map(flattenEvent);
  const headers =
    columns.length > 0
      ? ['occured_at', ...columns.filter((column) => column !== 'occured_at')]
      : [...new Set(flat.flatMap((row) => Object.keys(row)))];
  return flat.map((row) =>
    Object.fromEntries(
      headers.map((header) => {
        const value = row[header];
        return [header, typeof value === 'object' && value !== null ? JSON.stringify(value) : value];
      }),
    ),
  );
}

export default function DiscoverPage() {
  const scope = useSearchScope();
  const location = useLocation();
  const [more, setMore] = useState<MorePages | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [moreError, setMoreError] = useState<string | null>(null);
  const [exportAnchor, setExportAnchor] = useState<HTMLElement | null>(null);
  // The fields offered depend on the events loaded, which depend on the query,
  // so the loaded events are fed back in through this state.
  const [fieldDocs, setFieldDocs] = useState<EventDoc[]>(NO_DOCS);

  const sidebar = usePersistentSize('discover_sidebar_width', 272, 200, 560);
  const chart = usePersistentSize('discover_histogram_height', 132, 60, 420);

  const fields = useMemo(() => discoverFields(fieldDocs), [fieldDocs]);
  const discover = useDiscover(fields);
  const { state, update, built, range, searchKey } = discover;
  const query = built.ok ? built.query : null;

  const baseKey = scope && searchKey ? `${scope.projectId}:${searchKey}` : null;
  const eventsKey = baseKey ? `${baseKey}:${state.order}` : null;

  const first = useAsyncResult(eventsKey, async (signal) => {
    const page = await searchService.search(
      { ...scope!, signal },
      { query: query!, page_size: PAGE_SIZE, order: state.order },
    );
    setFieldDocs(page.items);
    return page;
  });
  const histogram = useAsyncResult(baseKey, (signal) =>
    searchService.histogram({ ...scope!, signal }, query!, range!.from, range!.to),
  );

  const extra = more && more.key === first.dataKey ? more : null;
  const docs = useMemo(
    () => (first.data ? [...first.data.items, ...(extra?.items ?? [])] : NO_DOCS),
    [first.data, extra],
  );
  const nextCursor = extra ? extra.next : (first.data?.next_cursor ?? null);
  const current = first.dataKey !== null && first.dataKey === eventsKey;
  const hits = histogram.data?.buckets.reduce((sum, bucket) => sum + bucket.count, 0);

  const loadMore = async () => {
    if (!scope || !query || !nextCursor || !first.dataKey) return;
    const key = first.dataKey;
    setLoadingMore(true);
    setMoreError(null);
    try {
      const page = await searchService.search(scope, {
        query,
        page_size: PAGE_SIZE,
        order: state.order,
        cursor: nextCursor,
      });
      const items = [...(extra?.items ?? []), ...page.items];
      setMore({ key, items, next: page.next_cursor });
      setFieldDocs([...(first.data?.items ?? []), ...items]);
    } catch (error) {
      if (!isAbort(error)) setMoreError(searchErrorMessage(error, 'Could not load more events'));
    } finally {
      setLoadingMore(false);
    }
  };

  const loadFacets = useCallback(
    (field: string, signal: AbortSignal) =>
      searchService.facets({ ...scope!, signal }, { query: query!, field }),
    [scope, query],
  );

  const lookupValues = useCallback(
    async (field: string, prefix: string): Promise<string[]> => {
      const definition = fields.find((candidate) => candidate.name === field);
      if (definition?.facetable && scope && query) {
        const facets = await searchService.facets(scope, {
          query,
          field,
          size: SUGGESTED_VALUES,
          prefix: definition.type === 'boolean' ? undefined : prefix,
        });
        return facets.buckets.map((bucket) => bucket.value);
      }
      // Fields the server cannot aggregate: suggest from the loaded events.
      return topValues(docs, field, 50)
        .buckets.map((bucket) => bucket.label)
        .filter((label) => label.toLowerCase().startsWith(prefix.toLowerCase()))
        .slice(0, SUGGESTED_VALUES);
    },
    [fields, scope, query, docs],
  );

  const toggleColumn = (field: string) =>
    update({
      columns: state.columns.includes(field)
        ? state.columns.filter((column) => column !== field)
        : [...state.columns, field],
    });
  const moveColumn = (field: string, offset: -1 | 1) => {
    const columns = [...state.columns];
    const index = columns.indexOf(field);
    const target = index + offset;
    if (index < 0 || target < 0 || target >= columns.length) return;
    [columns[index], columns[target]] = [columns[target], columns[index]];
    update({ columns });
  };
  const filterValue = (field: string, value: FilterValue, negate: boolean) =>
    update({ filters: addValueFilter(state.filters, field, value, negate) });
  const filterExists = (field: string) =>
    update({ filters: addExistsFilter(state.filters, field, false) });
  // The search rides along, so "back to sessions" returns to it.
  const sessionHref = (sessionId: string) =>
    `/app/discover/sessions/${encodeURIComponent(sessionId)}${location.search}`;

  if (!scope) {
    return (
      <EmptyStateCard
        heading="No project selected"
        subtext="Choose an organization and a project to search its events."
      />
    );
  }

  const busy = first.loading || histogram.loading;
  const error = first.error ?? histogram.error;

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <DiscoverHeader
        tab="events"
        discover={discover}
        fields={fields}
        loading={busy}
        lookupValues={lookupValues}
      />

      <Box sx={{ flex: 1, minHeight: 0, display: 'flex' }}>
        <FieldSidebar
          width={sidebar.size}
          fields={fields}
          columns={state.columns}
          docs={docs}
          searchKey={current ? baseKey : null}
          loadFacets={loadFacets}
          onToggleColumn={toggleColumn}
          onFilter={filterValue}
          onFilterExists={filterExists}
        />
        <ResizeHandle
          direction="column"
          label="Resize field list"
          size={sidebar.size}
          min={sidebar.min}
          max={sidebar.max}
          onResize={sidebar.setSize}
          onReset={sidebar.reset}
        />

        <Box sx={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
          <Box sx={{ ...panelSx, px: 2, pt: 1.25, pb: 0.5 }}>
            <Box sx={{ display: 'flex', alignItems: 'baseline', gap: 1.5, flexWrap: 'wrap' }}>
              <Typography sx={{ fontSize: '18px', fontWeight: 600, color: 'text.primary' }}>
                {hits === undefined ? '—' : formatCount(hits)}
                <Box component="span" sx={{ ml: 0.75, fontSize: '13px', fontWeight: 500, color: colorInk60 }}>
                  hit{hits === 1 ? '' : 's'}
                </Box>
              </Typography>
              {range ? (
                <Typography sx={{ fontSize: '12px', color: colorInk40 }}>
                  {formatTimestamp(range.from)} → {formatTimestamp(range.to)}
                  {histogram.data ? ` · per ${histogram.data.interval}` : ''}
                </Typography>
              ) : null}
            </Box>
            {histogram.data ? (
              <Histogram
                data={histogram.data}
                height={chart.size}
                onSelect={(from, to) =>
                  update({ time: { from: from.toISOString(), to: to.toISOString() } })
                }
              />
            ) : (
              <Skeleton variant="rounded" height={chart.size} sx={{ my: 0.5 }} />
            )}
          </Box>
          <ResizeHandle
            direction="row"
            label="Resize chart"
            size={chart.size}
            min={chart.min}
            max={chart.max}
            onResize={chart.setSize}
            onReset={chart.reset}
          />

          <Box sx={{ ...panelSx, flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
            {error ? (
              <Alert severity="error" sx={{ m: 1.5 }}>
                {error}
              </Alert>
            ) : null}

            {!first.data && first.loading ? (
              <Box sx={{ p: 2 }}>
                {[0, 1, 2, 3, 4, 5].map((row) => (
                  <Skeleton key={row} height={34} />
                ))}
              </Box>
            ) : null}

            {first.data && docs.length === 0 && !first.loading ? (
              <EmptyStateCard
                heading="No events match your search"
                subtext="Try a wider time range, fewer filters, or a different query."
              />
            ) : null}

            {docs.length > 0 ? (
              <>
                <Box sx={{ flex: 1, minHeight: 0, overflow: 'auto', opacity: first.loading ? 0.55 : 1 }}>
                  <DocTable
                    docs={docs}
                    order={state.order}
                    columns={state.columns}
                    onToggleOrder={() => update({ order: state.order === 'desc' ? 'asc' : 'desc' })}
                    onToggleColumn={toggleColumn}
                    onMoveColumn={moveColumn}
                    onFilter={filterValue}
                    onFilterExists={filterExists}
                    sessionHref={sessionHref}
                  />
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

                <Box
                  sx={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 1,
                    px: 1.5,
                    py: 0.5,
                    borderTop: HAIRLINE,
                  }}
                >
                  <Typography sx={{ fontSize: '12px', color: colorInk60 }}>
                    Showing {formatCount(docs.length)}
                    {hits !== undefined ? ` of ${formatCount(hits)}` : ''} events
                    {nextCursor ? '' : ' — all loaded'}
                  </Typography>
                  <Box sx={{ flex: 1 }} />
                  <Button
                    size="small"
                    startIcon={<DownloadIcon size={14} />}
                    onClick={(event) => setExportAnchor(event.currentTarget)}
                    sx={{ fontSize: '12px', py: 0.25 }}
                  >
                    Export
                  </Button>
                </Box>
              </>
            ) : null}
          </Box>
        </Box>
      </Box>

      <Menu anchorEl={exportAnchor} open={exportAnchor !== null} onClose={() => setExportAnchor(null)}>
        <MenuItem
          sx={{ fontSize: '13px' }}
          onClick={() => {
            downloadAsCSV(exportRows(docs, state.columns), 'events');
            setExportAnchor(null);
          }}
        >
          Loaded rows as CSV
        </MenuItem>
        <MenuItem
          sx={{ fontSize: '13px' }}
          onClick={() => {
            downloadAsJSON(docs, 'events');
            setExportAnchor(null);
          }}
        >
          Loaded rows as JSON
        </MenuItem>
      </Menu>
    </Box>
  );
}
