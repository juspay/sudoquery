import { useMemo } from 'react';
import { Box, ButtonBase, IconButton, Skeleton, Tooltip, Typography } from '@mui/material';
import { Minus as MinusIcon, Plus as PlusIcon } from 'lucide-react';
import { formatCount } from '../../discover/format';
import { isFilterValue } from '../../discover/filters';
import { topValues } from '../../discover/topValues';
import { useAsyncResult } from '../../discover/useDiscover';
import type { EventDoc, FieldDef, FilterValue } from '../../discover/types';
import type { Facets } from '../../services/searchService';
import { colorBlue, colorBluePale, colorInk40, colorInk60, colorRose } from '../../theme/tokens';
import { monoSx } from './styles';

interface Row {
  label: string;
  /** The value to filter on; `null` when it cannot go in a filter. */
  value: FilterValue | null;
  count: number;
}

/** Facet values arrive as text; filters need the field's own type. */
function typedValue(text: string, field: FieldDef): FilterValue {
  if (field.type === 'boolean') return text === 'true';
  if (field.type === 'number' && Number.isFinite(Number(text))) return Number(text);
  return text;
}

interface FieldDetailProps {
  field: FieldDef;
  /** The events loaded in the table. */
  docs: EventDoc[];
  /** Identifies the current search; `null` while there is none to aggregate. */
  searchKey: string | null;
  loadFacets: (field: string, signal: AbortSignal) => Promise<Facets>;
  onFilter: (field: string, value: FilterValue, negate: boolean) => void;
  onFilterExists: (field: string) => void;
}

export function FieldDetail({
  field,
  docs,
  searchKey,
  loadFacets,
  onFilter,
  onFilterExists,
}: FieldDetailProps) {
  const exact = field.facetable && searchKey !== null;
  const facets = useAsyncResult(exact ? `${searchKey}\n${field.name}` : null, (signal) =>
    loadFacets(field.name, signal),
  );
  const sample = useMemo(() => topValues(docs, field.name), [docs, field.name]);

  let rows: Row[];
  let total: number;
  let caption: string;
  if (exact && facets.data && facets.data.field === field.name) {
    rows = facets.data.buckets.map((bucket) => ({
      label: bucket.value,
      value: typedValue(bucket.value, field),
      count: bucket.count,
    }));
    total = facets.data.total;
    caption = `Exact, across ${formatCount(total)} matching event${total === 1 ? '' : 's'}`;
  } else {
    rows = sample.buckets.map((bucket) => ({
      label: bucket.label,
      value: isFilterValue(bucket.value) ? bucket.value : null,
      count: bucket.count,
    }));
    total = sample.withField;
    caption = `From the ${formatCount(sample.sampled)} loaded event${sample.sampled === 1 ? '' : 's'}; present in ${formatCount(sample.withField)}`;
  }

  if (exact && facets.loading && !facets.data) {
    return (
      <Box sx={{ px: 1.5, pb: 1.5 }}>
        <Skeleton height={20} />
        <Skeleton height={20} width="80%" />
        <Skeleton height={20} width="60%" />
      </Box>
    );
  }

  return (
    <Box sx={{ px: 1.5, pb: 1.5 }}>
      <Typography sx={{ fontSize: '11px', fontWeight: 600, color: colorInk60, mb: 0.5 }}>
        {rows.length > 1 ? `Top ${rows.length} values` : 'Top values'}
      </Typography>

      {exact && facets.error ? (
        <Typography sx={{ fontSize: '11px', color: colorRose, mb: 0.5 }}>{facets.error}</Typography>
      ) : null}
      {rows.length === 0 ? (
        <Typography sx={{ fontSize: '11px', color: colorInk40 }}>
          No values for this field in {exact ? 'the matching events' : 'the loaded events'}.
        </Typography>
      ) : null}

      {rows.map((row) => {
        const share = total > 0 ? row.count / total : 0;
        const value = row.value;
        return (
          <Box key={row.label} sx={{ mb: 0.75 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
              <Typography
                title={row.label}
                sx={{
                  ...monoSx,
                  fontSize: '11.5px',
                  flex: 1,
                  minWidth: 0,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  color: 'text.primary',
                }}
              >
                {row.label}
              </Typography>
              <Typography
                title={`${formatCount(row.count)} events`}
                sx={{ fontSize: '11px', color: colorInk60, fontVariantNumeric: 'tabular-nums' }}
              >
                {(share * 100).toFixed(1)}%
              </Typography>
              <Tooltip title="Filter for value">
                <span>
                  <IconButton
                    size="small"
                    aria-label={`Filter for ${field.name}: ${row.label}`}
                    disabled={value === null}
                    onClick={() => value !== null && onFilter(field.name, value, false)}
                    sx={{ p: 0.25 }}
                  >
                    <PlusIcon size={12} />
                  </IconButton>
                </span>
              </Tooltip>
              <Tooltip title="Filter out value">
                <span>
                  <IconButton
                    size="small"
                    aria-label={`Filter out ${field.name}: ${row.label}`}
                    disabled={value === null}
                    onClick={() => value !== null && onFilter(field.name, value, true)}
                    sx={{ p: 0.25 }}
                  >
                    <MinusIcon size={12} />
                  </IconButton>
                </span>
              </Tooltip>
            </Box>
            <Box sx={{ height: 3, borderRadius: 2, bgcolor: colorBluePale, overflow: 'hidden' }}>
              <Box sx={{ width: `${Math.min(100, share * 100)}%`, height: '100%', bgcolor: colorBlue }} />
            </Box>
          </Box>
        );
      })}

      <Typography sx={{ fontSize: '10.5px', color: colorInk40, mt: 0.75, lineHeight: 1.4 }}>
        {caption}
      </Typography>
      <ButtonBase
        onClick={() => onFilterExists(field.name)}
        sx={{ mt: 0.5, fontSize: '11px', fontWeight: 600, color: colorBlue }}
      >
        Filter for field present
      </ButtonBase>
    </Box>
  );
}
