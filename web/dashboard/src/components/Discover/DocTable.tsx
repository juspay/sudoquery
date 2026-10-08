import { memo, useMemo, useState } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { Box, ButtonBase, IconButton, Tooltip } from '@mui/material';
import {
  ArrowDown as ArrowDownIcon,
  ArrowUp as ArrowUpIcon,
  ChevronDown as CollapseIcon,
  ChevronLeft as MoveLeftIcon,
  ChevronRight as ExpandIcon,
  X as RemoveIcon,
} from 'lucide-react';
import { TIME_FIELD, flattenEvent } from '../../discover/fields';
import { formatTimestamp } from '../../discover/timeRange';
import { valueLabel } from '../../discover/topValues';
import type { EventDoc, SortOrder } from '../../discover/types';
import { colorBlue, colorCream, colorCream2, colorInk40, colorInk60 } from '../../theme/tokens';
import { DocDetail, type DocActions } from './DocDetail';
import { HAIRLINE, monoSx, overlineSx } from './styles';

/** Fields shown first in the document summary, when present. */
const SUMMARY_FIRST = ['name', 'session_id', 'anon_id', 'actor_id', 'source'];
/** Fields the summary leaves out: shown elsewhere or the same on every row. */
const SUMMARY_SKIP = new Set([TIME_FIELD, 'project_id', 'envelop_version']);

const TIME_WIDTH = 200;
const COLUMN_MIN_WIDTH = 160;

function docTime(doc: EventDoc): string {
  const raw = doc[TIME_FIELD];
  const date = typeof raw === 'string' ? new Date(raw) : null;
  return date && !Number.isNaN(date.getTime()) ? formatTimestamp(date) : '—';
}

function docKey(doc: EventDoc, index: number): string {
  return typeof doc.id === 'string' ? doc.id : String(index);
}

/** What an event says about itself comes before the bookkeeping around it. */
function summaryRank(field: string): number {
  if (field.startsWith('properties.')) return 0;
  if (field.startsWith('system_properties.')) return 1;
  return 2;
}

function summaryEntries(flat: Record<string, unknown>): Array<[string, unknown]> {
  const first = SUMMARY_FIRST.filter((field) => field in flat).map(
    (field): [string, unknown] => [field, flat[field]],
  );
  const rest = Object.entries(flat)
    .filter(([field]) => !SUMMARY_FIRST.includes(field) && !SUMMARY_SKIP.has(field))
    .sort(([a], [b]) => summaryRank(a) - summaryRank(b) || a.localeCompare(b));
  return [...first, ...rest];
}

interface DocRowProps extends DocActions {
  doc: EventDoc;
  template: string;
  expanded: boolean;
  onToggle: () => void;
}

const DocRow = memo(function DocRow({ doc, template, expanded, onToggle, ...actions }: DocRowProps) {
  const flat = useMemo(() => flattenEvent(doc), [doc]);
  const { columns, sessionHref } = actions;

  return (
    <Box sx={{ borderBottom: HAIRLINE }}>
      <Box
        onClick={onToggle}
        sx={{
          display: 'grid',
          gridTemplateColumns: template,
          alignItems: 'start',
          cursor: 'pointer',
          bgcolor: expanded ? colorCream : 'transparent',
          '&:hover': { bgcolor: colorCream },
        }}
      >
        <Box sx={{ py: 0.75, pl: 1, color: colorInk60, lineHeight: 0 }}>
          {expanded ? <CollapseIcon size={15} /> : <ExpandIcon size={15} />}
        </Box>
        <Box sx={{ ...monoSx, py: 0.75, pr: 1.5, whiteSpace: 'nowrap' }}>{docTime(doc)}</Box>

        {columns.length === 0 ? (
          <Box sx={{ py: 0.75, pr: 1.5, minWidth: 0 }}>
            {/* The clamp sits on an unpadded box: padding would show a slice of the next line. */}
            <Box
              sx={{
                ...monoSx,
                display: '-webkit-box',
                WebkitLineClamp: 3,
                WebkitBoxOrient: 'vertical',
                overflow: 'hidden',
                overflowWrap: 'anywhere',
              }}
            >
              {summaryEntries(flat).map(([field, value]) => (
                <Box component="span" key={field} sx={{ mr: 1.5 }}>
                  <Box
                    component="span"
                    sx={{
                      px: 0.5,
                      mr: 0.5,
                      borderRadius: '3px',
                      bgcolor: colorCream2,
                      color: colorInk60,
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {field}:
                  </Box>
                  {valueLabel(value)}
                </Box>
              ))}
            </Box>
          </Box>
        ) : (
          columns.map((column) => {
            const value = flat[column];
            const linked = column === 'session_id' && typeof value === 'string' && sessionHref;
            return (
              <Box
                key={column}
                sx={{
                  ...monoSx,
                  py: 0.75,
                  pr: 1.5,
                  minWidth: 0,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  color: column in flat ? 'text.primary' : colorInk40,
                }}
                title={column in flat ? valueLabel(value) : undefined}
              >
                {linked ? (
                  <Box
                    component={RouterLink}
                    to={linked(value)}
                    onClick={(event: React.MouseEvent) => event.stopPropagation()}
                    sx={{ color: colorBlue, textDecoration: 'none', '&:hover': { textDecoration: 'underline' } }}
                  >
                    {value}
                  </Box>
                ) : column in flat ? (
                  valueLabel(value)
                ) : (
                  '—'
                )}
              </Box>
            );
          })
        )}
      </Box>
      {expanded ? <DocDetail doc={doc} {...actions} /> : null}
    </Box>
  );
});

interface DocTableProps extends DocActions {
  onToggleColumn: (field: string) => void;
  docs: EventDoc[];
  order: SortOrder;
  onToggleOrder?: () => void;
  onMoveColumn?: (field: string, offset: -1 | 1) => void;
}

export function DocTable({ docs, order, onToggleOrder, onMoveColumn, ...actions }: DocTableProps) {
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const { columns, onToggleColumn } = actions;

  const template =
    columns.length === 0
      ? `32px ${TIME_WIDTH}px minmax(0, 1fr)`
      : `32px ${TIME_WIDTH}px repeat(${columns.length}, minmax(${COLUMN_MIN_WIDTH}px, 1fr))`;
  const minWidth =
    columns.length === 0 ? undefined : 32 + TIME_WIDTH + columns.length * COLUMN_MIN_WIDTH;

  const toggle = (key: string) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (!next.delete(key)) next.add(key);
      return next;
    });

  const SortIcon = order === 'desc' ? ArrowDownIcon : ArrowUpIcon;

  return (
    <Box sx={{ minWidth }}>
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: template,
          alignItems: 'center',
          position: 'sticky',
          top: 0,
          zIndex: 1,
          bgcolor: '#FFFFFF',
          borderBottom: HAIRLINE,
        }}
      >
        <Box />
        <Box sx={{ py: 1 }}>
          <Tooltip title={onToggleOrder ? `Sorted ${order === 'desc' ? 'newest' : 'oldest'} first — click to reverse` : ''}>
            <ButtonBase
              onClick={onToggleOrder}
              disabled={!onToggleOrder}
              sx={{ ...overlineSx, gap: 0.5 }}
            >
              Time
              <SortIcon size={12} />
            </ButtonBase>
          </Tooltip>
        </Box>
        {columns.length === 0 ? (
          <Box sx={{ ...overlineSx, py: 1 }}>Document</Box>
        ) : (
          columns.map((column, index) => (
            <Box
              key={column}
              sx={{
                display: 'flex',
                alignItems: 'center',
                minWidth: 0,
                pr: 1,
                '&:hover .column-actions': { opacity: 1 },
              }}
            >
              <Box
                sx={{
                  ...monoSx,
                  fontWeight: 600,
                  py: 1,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}
                title={column}
              >
                {column}
              </Box>
              <Box className="column-actions" sx={{ display: 'flex', opacity: 0, transition: 'opacity 0.12s' }}>
                {onMoveColumn && index > 0 ? (
                  <Tooltip title="Move left">
                    <IconButton
                      size="small"
                      aria-label={`Move ${column} left`}
                      onClick={() => onMoveColumn(column, -1)}
                    >
                      <MoveLeftIcon size={12} />
                    </IconButton>
                  </Tooltip>
                ) : null}
                {onMoveColumn && index < columns.length - 1 ? (
                  <Tooltip title="Move right">
                    <IconButton
                      size="small"
                      aria-label={`Move ${column} right`}
                      onClick={() => onMoveColumn(column, 1)}
                    >
                      <ExpandIcon size={12} />
                    </IconButton>
                  </Tooltip>
                ) : null}
                <Tooltip title="Remove column">
                  <IconButton
                    size="small"
                    aria-label={`Remove column ${column}`}
                    onClick={() => onToggleColumn(column)}
                  >
                    <RemoveIcon size={12} />
                  </IconButton>
                </Tooltip>
              </Box>
            </Box>
          ))
        )}
      </Box>

      {docs.map((doc, index) => {
        const key = docKey(doc, index);
        return (
          <DocRow
            key={key}
            doc={doc}
            template={template}
            expanded={expanded.has(key)}
            onToggle={() => toggle(key)}
            {...actions}
          />
        );
      })}
    </Box>
  );
}
