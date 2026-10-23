import { useMemo, useState } from 'react';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Chip,
  IconButton,
  MenuItem,
  Skeleton,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { Trash2 as DeleteIcon } from 'lucide-react';
import { CustomersHeader } from '../components/Customers/CustomersHeader';
import { HAIRLINE, monoSx, overlineSx, panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { useAsyncResult } from '../discover/useDiscover';
import { useProject } from '../contexts/ProjectContext';
import { apiErrorMessage } from '../services/apiClient';
import { cdpService, type NewMetric, type ValueKind } from '../services/cdpService';
import { eventService } from '../services/eventService';
import { colorInk60 } from '../theme/tokens';

const NUMERIC_TYPES = /^(U?Int\d+|Float\d+|Decimal.*)$/;

/** Catalog properties live under `properties`; the CDP addresses them as `properties.<path>`. */
const toPath = (property: string) => `properties.${property}`;

const KIND_LABELS: Record<ValueKind, string> = {
  count: 'Count of events',
  property_number: 'Sum of a number property',
  occurred_at_epoch: 'Time of the latest event',
  property_string: 'Latest text value',
};

function Panel({ title, hint, children }: { title: string; hint: string; children: React.ReactNode }) {
  return (
    <Box sx={{ ...panelSx, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      <Box sx={{ px: 2, py: 1.25, borderBottom: HAIRLINE }}>
        <Typography sx={{ ...overlineSx }}>{title}</Typography>
        <Typography sx={{ fontSize: '12.5px', color: colorInk60 }}>{hint}</Typography>
      </Box>
      {children}
    </Box>
  );
}

function ListRow({ children, onDelete, busy }: { children: React.ReactNode; onDelete: () => void; busy: boolean }) {
  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, px: 2, py: 0.75, borderBottom: HAIRLINE }}>
      <Box sx={{ flex: 1, minWidth: 0 }}>{children}</Box>
      <Tooltip title="Delete">
        <span>
          <IconButton size="small" onClick={onDelete} disabled={busy} aria-label="Delete">
            <DeleteIcon size={15} />
          </IconButton>
        </span>
      </Tooltip>
    </Box>
  );
}

/**
 * The organization's CDP vocabulary: which events feed profiles, and which
 * per-customer metrics are folded from them. Shared by every project of the
 * organization; changing it needs organization admin.
 */
export default function CustomersSetupPage() {
  const { currentProject } = useProject();
  const [reload, setReload] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const key = currentProject && currentProject.status === 'ACTIVE' ? `${currentProject.id}:${reload}` : null;
  const events = useAsyncResult(key, () => cdpService.eventTypes());
  const metrics = useAsyncResult(key, () => cdpService.metricDefinitions());
  // Suggestions: the events and properties this project has actually sent.
  const catalog = useAsyncResult(key, () => eventService.getEventSchemaCatalog());

  const [eventName, setEventName] = useState('');
  const [dedupPath, setDedupPath] = useState('');
  const [metric, setMetric] = useState<NewMetric>({ metric_name: '', event_name: '', value_kind: 'count' });

  if (!currentProject) {
    return <EmptyStateCard heading="No project selected" subtext="Choose an organization and a project." />;
  }

  const run = async (action: () => Promise<unknown>, fallback: string) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      setReload((n) => n + 1);
      return true;
    } catch (e) {
      setError(apiErrorMessage(e, fallback));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const addEvent = async () => {
    const name = eventName.trim();
    const path = dedupPath.trim();
    const dedup = path ? { entity: path.split('.').pop() ?? path, keyPath: path } : undefined;
    if (await run(() => cdpService.putEventType(name, dedup), 'Could not add the event')) {
      setEventName('');
      setDedupPath('');
    }
  };

  const needsPath = metric.value_kind === 'property_number' || metric.value_kind === 'property_string';
  const addMetric = async () => {
    const body: NewMetric = {
      metric_name: metric.metric_name.trim(),
      event_name: metric.event_name,
      value_kind: metric.value_kind,
      display_name: metric.display_name?.trim() || undefined,
      value_path: needsPath ? metric.value_path?.trim() : undefined,
      window_days: metric.window_days || undefined,
      aggregation: metric.value_kind === 'property_string' ? 'latest' : undefined,
    };
    if (await run(() => cdpService.putMetric(body), 'Could not add the metric')) {
      setMetric({ metric_name: '', event_name: metric.event_name, value_kind: 'count' });
    }
  };

  const eventRows = events.data ?? [];
  const catalogRows = catalog.data;

  const catalogEvents = useMemo(() => [...new Set((catalogRows ?? []).map((r) => r.event_name))].sort(), [catalogRows]);
  const propertiesOf = (event: string, numericOnly = false) =>
    (catalogRows ?? [])
      .filter((r) => r.event_name === event && (!numericOnly || NUMERIC_TYPES.test(r.type)))
      .map((r) => toPath(r.property));
  const declared = new Set(eventRows.map((e) => e.event_name));
  const suggestedEvents = catalogEvents.filter((n) => !declared.has(n));
  const metricRows = metrics.data ?? [];

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <CustomersHeader tab="setup" />
      <Typography sx={{ fontSize: '12.5px', color: colorInk60 }}>
        Profiles are built only from the events declared here, and segments use the metrics defined here. Both are
        shared by every project in this organization; changing them needs organization admin. Changes take up to a
        minute to apply: events that arrive before then are not counted toward them.
      </Typography>
      {error ? <Alert severity="error">{error}</Alert> : null}
      {events.error || metrics.error ? <Alert severity="error">{events.error ?? metrics.error}</Alert> : null}

      <Box
        sx={{
          flex: 1,
          minHeight: 0,
          display: 'grid',
          gridTemplateColumns: { xs: '1fr', lg: '2fr 3fr' },
          gap: 1.5,
          overflow: 'auto',
        }}
      >
        <Panel title="Events" hint="Event names that feed customer profiles.">
          <Box
            component="form"
            onSubmit={(e) => {
              e.preventDefault();
              void addEvent();
            }}
            sx={{ display: 'flex', gap: 1, p: 1.5, borderBottom: HAIRLINE }}
          >
            <Autocomplete
              freeSolo
              size="small"
              options={suggestedEvents}
              inputValue={eventName}
              onInputChange={(_, v) => setEventName(v)}
              renderInput={(params) => <TextField {...params} label="Event name" />}
              sx={{ flex: 1 }}
            />
            <Autocomplete
              freeSolo
              size="small"
              options={[...new Set(propertiesOf(eventName.trim()))]}
              inputValue={dedupPath}
              onInputChange={(_, v) => setDedupPath(v)}
              renderInput={(params) => (
                <TextField {...params} label="Dedup on (optional)" placeholder="properties.order_id" />
              )}
              sx={{ flex: 1 }}
            />
            <Button type="submit" variant="outlined" size="small" disabled={busy || !eventName.trim()}>
              Add
            </Button>
          </Box>
          {suggestedEvents.length > 0 ? (
            <Box sx={{ px: 1.5, py: 1, borderBottom: HAIRLINE, display: 'flex', flexWrap: 'wrap', gap: 0.5, alignItems: 'center' }}>
              <Typography sx={{ fontSize: '12px', color: colorInk60, mr: 0.5 }}>Seen in your data:</Typography>
              {suggestedEvents.slice(0, 12).map((n) => (
                <Chip key={n} size="small" label={n} disabled={busy} onClick={() => setEventName(n)} />
              ))}
            </Box>
          ) : null}
          {!events.data && events.loading ? <Skeleton height={34} sx={{ mx: 2 }} /> : null}
          {events.data && eventRows.length === 0 ? (
            <Typography sx={{ px: 2, py: 1.5, fontSize: '13px', color: colorInk60 }}>
              No events declared yet — nothing feeds profiles until you add one.
            </Typography>
          ) : null}
          {eventRows.map((e) => (
            <ListRow
              key={e.event_name}
              busy={busy}
              onDelete={() => void run(() => cdpService.deleteEventType(e.event_name), 'Could not delete the event')}
            >
              <Typography sx={monoSx}>{e.event_name}</Typography>
              {e.dedup_key_paths?.length ? (
                <Typography sx={{ fontSize: '12px', color: colorInk60 }}>
                  deduplicated on {e.dedup_key_paths.join(', ')}
                </Typography>
              ) : null}
            </ListRow>
          ))}
        </Panel>

        <Panel title="Metrics" hint="Per-customer numbers folded from events; segments are rules over these.">
          <Box
            component="form"
            onSubmit={(e) => {
              e.preventDefault();
              void addMetric();
            }}
            sx={{ display: 'flex', gap: 1, p: 1.5, flexWrap: 'wrap', borderBottom: HAIRLINE }}
          >
            <TextField
              size="small"
              label="Metric name"
              placeholder="total_spend"
              value={metric.metric_name}
              onChange={(e) => setMetric({ ...metric, metric_name: e.target.value })}
              sx={{ flex: '1 1 150px' }}
            />
            <TextField
              size="small"
              label="Label"
              placeholder="Total spend"
              value={metric.display_name ?? ''}
              onChange={(e) => setMetric({ ...metric, display_name: e.target.value })}
              sx={{ flex: '1 1 130px' }}
            />
            <TextField
              select
              size="small"
              label="From event"
              value={metric.event_name}
              onChange={(e) => setMetric({ ...metric, event_name: e.target.value })}
              sx={{ flex: '1 1 150px' }}
            >
              {eventRows.map((e) => (
                <MenuItem key={e.event_name} value={e.event_name}>
                  {e.event_name}
                </MenuItem>
              ))}
            </TextField>
            <TextField
              select
              size="small"
              label="Value"
              value={metric.value_kind}
              onChange={(e) => setMetric({ ...metric, value_kind: e.target.value as ValueKind })}
              sx={{ flex: '1 1 190px' }}
            >
              {(Object.keys(KIND_LABELS) as ValueKind[]).map((k) => (
                <MenuItem key={k} value={k}>
                  {KIND_LABELS[k]}
                </MenuItem>
              ))}
            </TextField>
            {needsPath ? (
              <Autocomplete
                freeSolo
                size="small"
                options={[...new Set(propertiesOf(metric.event_name, metric.value_kind === 'property_number'))]}
                inputValue={metric.value_path ?? ''}
                onInputChange={(_, v) => setMetric({ ...metric, value_path: v })}
                renderInput={(params) => <TextField {...params} label="Property path" placeholder="properties.amount" />}
                sx={{ flex: '1 1 150px' }}
              />
            ) : null}
            {metric.value_kind === 'count' || metric.value_kind === 'property_number' ? (
              <TextField
                size="small"
                label="Last N days"
                placeholder="all time"
                value={metric.window_days ?? ''}
                onChange={(e) => {
                  const n = Number.parseInt(e.target.value, 10);
                  setMetric({ ...metric, window_days: Number.isFinite(n) && n > 0 ? n : undefined });
                }}
                inputProps={{ inputMode: 'numeric' }}
                sx={{ flex: '0 1 110px' }}
              />
            ) : null}
            <Button
              type="submit"
              variant="outlined"
              size="small"
              disabled={busy || !metric.metric_name.trim() || !metric.event_name || (needsPath && !metric.value_path?.trim())}
            >
              Add
            </Button>
          </Box>
          {!metrics.data && metrics.loading ? <Skeleton height={34} sx={{ mx: 2 }} /> : null}
          {metrics.data && metricRows.length === 0 ? (
            <Typography sx={{ px: 2, py: 1.5, fontSize: '13px', color: colorInk60 }}>No metrics yet.</Typography>
          ) : null}
          {metricRows.map((m) => (
            <ListRow
              key={`${m.metric_name}:${m.event_name}:${m.workspace_id}`}
              busy={busy}
              onDelete={() => void run(() => cdpService.deleteMetric(m.metric_name, m.event_name), 'Could not delete the metric')}
            >
              <Typography sx={{ fontSize: '13px', fontWeight: 600 }}>
                {m.display_name ?? m.metric_name}{' '}
                <Box component="span" sx={{ ...monoSx, color: colorInk60, fontWeight: 400 }}>
                  {m.metric_name}
                </Box>
              </Typography>
              <Typography sx={{ fontSize: '12px', color: colorInk60 }}>
                {KIND_LABELS[m.value_kind as ValueKind] ?? m.value_kind}
                {m.value_path ? ` · ${m.value_path}` : ''} · from {m.event_name}
                {m.window_days ? ` · last ${m.window_days} days` : ''}
              </Typography>
            </ListRow>
          ))}
        </Panel>
      </Box>
    </Box>
  );
}
