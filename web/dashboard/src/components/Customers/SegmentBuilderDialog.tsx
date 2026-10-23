import { useEffect, useMemo, useState } from 'react';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  MenuItem,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material';
import { Plus as PlusIcon, X as XIcon } from 'lucide-react';
import {
  OP_LABELS,
  emptyCondition,
  metricChoices,
  opsFor,
  slugify,
  toPredicate,
  type Condition,
  type Rule,
} from '../../cdp/predicate';
import { formatCount } from '../../cdp/format';
import { apiErrorMessage } from '../../services/apiClient';
import { cdpService, type MetricDefinition, type SegmentPreview } from '../../services/cdpService';
import { colorInk60 } from '../../theme/tokens';

const PREVIEW_DEBOUNCE_MS = 400;

interface SegmentBuilderDialogProps {
  open: boolean;
  onClose: () => void;
  onCreated: (segmentId: string) => void;
}

const VALUELESS = new Set(['exists', 'missing']);

function ConditionRow({
  condition,
  metrics,
  onChange,
  onRemove,
}: {
  condition: Condition;
  metrics: MetricDefinition[];
  onChange: (c: Condition) => void;
  onRemove: () => void;
}) {
  const kind = metrics.find((m) => m.metric_name === condition.metric)?.value_kind;
  const ops = opsFor(kind);
  const days = condition.op === 'within_days' || condition.op === 'beyond_days';
  return (
    <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
      <TextField
        select
        size="small"
        label="Metric"
        value={condition.metric}
        onChange={(e) => {
          const metric = e.target.value;
          const nextOps = opsFor(metrics.find((m) => m.metric_name === metric)?.value_kind);
          onChange({ ...condition, metric, op: nextOps.includes(condition.op) ? condition.op : nextOps[0] });
        }}
        sx={{ flex: 2, minWidth: 180 }}
      >
        {metrics.map((m) => (
          <MenuItem key={m.metric_name} value={m.metric_name}>
            {m.display_name ?? m.metric_name}
          </MenuItem>
        ))}
      </TextField>
      <TextField
        select
        size="small"
        label="Condition"
        value={condition.op}
        onChange={(e) => onChange({ ...condition, op: e.target.value as Condition['op'] })}
        sx={{ flex: 1.5, minWidth: 170 }}
      >
        {ops.map((op) => (
          <MenuItem key={op} value={op}>
            {OP_LABELS[op]}
          </MenuItem>
        ))}
      </TextField>
      {VALUELESS.has(condition.op) ? (
        <Box sx={{ flex: 1 }} />
      ) : (
        <TextField
          size="small"
          label={days ? 'Days' : condition.op === 'between' ? 'From' : 'Value'}
          value={condition.value}
          onChange={(e) => onChange({ ...condition, value: e.target.value })}
          inputProps={{ inputMode: 'decimal' }}
          sx={{ flex: 1, minWidth: 90 }}
        />
      )}
      {condition.op === 'between' ? (
        <TextField
          size="small"
          label="To"
          value={condition.valueTo}
          onChange={(e) => onChange({ ...condition, valueTo: e.target.value })}
          inputProps={{ inputMode: 'decimal' }}
          sx={{ flex: 1, minWidth: 90 }}
        />
      ) : null}
      <IconButton size="small" onClick={onRemove} aria-label="Remove condition">
        <XIcon size={16} />
      </IconButton>
    </Box>
  );
}

/** Name a segment and compose its rule, with a live count of who matches. */
export function SegmentBuilderDialog({ open, onClose, onCreated }: SegmentBuilderDialogProps) {
  const [definitions, setDefinitions] = useState<MetricDefinition[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [segmentId, setSegmentId] = useState('');
  const [idEdited, setIdEdited] = useState(false);
  const [rule, setRule] = useState<Rule>({ match: 'all', conditions: [emptyCondition()] });
  const [preview, setPreview] = useState<{ result?: SegmentPreview; error?: string; loading: boolean }>({
    loading: false,
  });
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || definitions) return;
    cdpService.metricDefinitions().then(setDefinitions, (e: unknown) =>
      setLoadError(apiErrorMessage(e, 'Could not load metrics')),
    );
  }, [open, definitions]);

  const metrics = useMemo(() => metricChoices(definitions ?? []), [definitions]);
  const built = useMemo(() => toPredicate(rule), [rule]);
  const id = idEdited ? segmentId : slugify(name);

  // Debounced dry-run count while the rule is valid.
  const predicateKey = 'predicate' in built ? JSON.stringify(built.predicate) : null;
  useEffect(() => {
    if (!open || predicateKey === null) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      setPreview((p) => ({ ...p, loading: true }));
      cdpService.preview(JSON.parse(predicateKey), controller.signal).then(
        (result) => setPreview({ result, loading: false }),
        (e: unknown) => {
          if (!controller.signal.aborted) setPreview({ error: apiErrorMessage(e, 'Preview failed'), loading: false });
        },
      );
    }, PREVIEW_DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [open, predicateKey]);

  const setCondition = (index: number, c: Condition) =>
    setRule((r) => ({ ...r, conditions: r.conditions.map((old, i) => (i === index ? c : old)) }));

  const create = async () => {
    if (!('predicate' in built) || !id || !name.trim()) return;
    setSaving(true);
    setSaveError(null);
    try {
      await cdpService.createSegment(id, { display_name: name.trim(), predicate: built.predicate });
      onCreated(id);
    } catch (e) {
      setSaveError(apiErrorMessage(e, 'Could not create the segment'));
    } finally {
      setSaving(false);
    }
  };

  const canSave = 'predicate' in built && id.length > 0 && name.trim().length > 0 && !saving;

  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle>New segment</DialogTitle>
      <DialogContent sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: '8px !important' }}>
        {loadError ? <Alert severity="error">{loadError}</Alert> : null}
        <Box sx={{ display: 'flex', gap: 1.5 }}>
          <TextField
            size="small"
            label="Name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            sx={{ flex: 2 }}
            autoFocus
          />
          <TextField
            size="small"
            label="Segment id"
            value={id}
            onChange={(e) => {
              setIdEdited(true);
              setSegmentId(e.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ''));
            }}
            helperText="Permanent — a rule change is a new segment"
            sx={{ flex: 1 }}
          />
        </Box>

        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <Typography sx={{ fontSize: '13px' }}>Profiles matching</Typography>
          <ToggleButtonGroup
            size="small"
            exclusive
            value={rule.match}
            onChange={(_, v: Rule['match'] | null) => v && setRule((r) => ({ ...r, match: v }))}
            sx={{ '& .MuiToggleButton-root': { py: 0.25, px: 1.25, fontSize: '12px', textTransform: 'none' } }}
          >
            <ToggleButton value="all">all</ToggleButton>
            <ToggleButton value="any">any</ToggleButton>
          </ToggleButtonGroup>
          <Typography sx={{ fontSize: '13px' }}>of these conditions</Typography>
        </Box>

        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.25 }}>
          {rule.conditions.map((c, i) => (
            <ConditionRow
              key={i}
              condition={c}
              metrics={metrics}
              onChange={(next) => setCondition(i, next)}
              onRemove={() => setRule((r) => ({ ...r, conditions: r.conditions.filter((_, j) => j !== i) }))}
            />
          ))}
          <Box>
            <Button
              size="small"
              startIcon={<PlusIcon size={14} />}
              onClick={() => setRule((r) => ({ ...r, conditions: [...r.conditions, emptyCondition()] }))}
            >
              Add condition
            </Button>
          </Box>
        </Box>

        <Box sx={{ minHeight: 24 }}>
          {'error' in built ? (
            <Typography sx={{ fontSize: '12.5px', color: colorInk60 }}>{built.error}</Typography>
          ) : preview.error ? (
            <Alert severity="warning">{preview.error}</Alert>
          ) : preview.result ? (
            <Typography sx={{ fontSize: '13px', opacity: preview.loading ? 0.5 : 1 }}>
              <strong>{formatCount(preview.result.matching_customers)}</strong>
              {preview.result.truncated ? '+' : ''} of {formatCount(preview.result.workspace_members)} profiles match
              {preview.result.matching_breakdown
                ? ` (${formatCount(preview.result.matching_breakdown.customers)} customers, ${formatCount(preview.result.matching_breakdown.visitors)} visitors)`
                : ''}
              {preview.result.is_temporal ? ' · time-based: membership updates as days pass' : ''}
            </Typography>
          ) : null}
        </Box>
        {saveError ? <Alert severity="error">{saveError}</Alert> : null}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="contained" onClick={() => void create()} disabled={!canSave}>
          Create segment
        </Button>
      </DialogActions>
    </Dialog>
  );
}
