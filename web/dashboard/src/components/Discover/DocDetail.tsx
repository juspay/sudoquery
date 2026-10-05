import { useMemo, useState } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { Box, Button, IconButton, Tab, Tabs, Tooltip } from '@mui/material';
import {
  Asterisk as ExistsIcon,
  Check as CheckIcon,
  Columns3 as ColumnIcon,
  Copy as CopyIcon,
  Minus as MinusIcon,
  Plus as PlusIcon,
} from 'lucide-react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneLight } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { flattenEvent } from '../../discover/fields';
import { isFilterValue } from '../../discover/filters';
import { valueLabel } from '../../discover/topValues';
import type { EventDoc, FilterValue } from '../../discover/types';
import { colorBlue, colorCream, colorInk40, colorInk60 } from '../../theme/tokens';
import { safeStringify } from '../../utils/cellValue';
import { HAIRLINE, monoSx } from './styles';

export interface DocActions {
  /** Columns currently shown in the table. */
  columns: string[];
  /** Omit where there is no table to add columns to. */
  onToggleColumn?: (field: string) => void;
  onFilter: (field: string, value: FilterValue, negate: boolean) => void;
  onFilterExists: (field: string) => void;
  /** Where the session viewer for a session id lives; omit to hide the link. */
  sessionHref?: (sessionId: string) => string;
}

interface DocDetailProps extends DocActions {
  doc: EventDoc;
}

export function DocDetail({
  doc,
  columns,
  onToggleColumn,
  onFilter,
  onFilterExists,
  sessionHref,
}: DocDetailProps) {
  const [view, setView] = useState<'table' | 'json'>('table');
  const [copied, setCopied] = useState(false);

  const rows = useMemo(
    () => Object.entries(flattenEvent(doc)).sort(([a], [b]) => a.localeCompare(b)),
    [doc],
  );
  const json = useMemo(() => safeStringify(doc, 2), [doc]);
  const sessionId = typeof doc.session_id === 'string' ? doc.session_id : null;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(json);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard unavailable (e.g. non-secure context) — nothing sensible to do
    }
  };

  return (
    <Box sx={{ bgcolor: colorCream, borderTop: HAIRLINE, px: 2, pb: 2 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        <Tabs
          value={view}
          onChange={(_, next: 'table' | 'json') => setView(next)}
          sx={{ minHeight: 36, '& .MuiTab-root': { minHeight: 36, fontSize: '12px', py: 0 } }}
        >
          <Tab value="table" label="Table" />
          <Tab value="json" label="JSON" />
        </Tabs>
        <Box sx={{ flex: 1 }} />
        {sessionId && sessionHref ? (
          <Button
            component={RouterLink}
            to={sessionHref(sessionId)}
            size="small"
            sx={{ fontSize: '12px', py: 0.25 }}
          >
            Open session
          </Button>
        ) : null}
        <Tooltip title={copied ? 'Copied' : 'Copy JSON'}>
          <IconButton size="small" aria-label="Copy JSON" onClick={copy}>
            {copied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
          </IconButton>
        </Tooltip>
      </Box>

      {view === 'table' ? (
        <Box sx={{ bgcolor: '#FFFFFF', border: HAIRLINE, borderRadius: '4px' }}>
          {rows.map(([field, value]) => {
            const filterable = isFilterValue(value);
            const shown = columns.includes(field);
            return (
              <Box
                key={field}
                sx={{
                  display: 'grid',
                  gridTemplateColumns: '112px minmax(160px, 280px) 1fr',
                  alignItems: 'start',
                  borderBottom: HAIRLINE,
                  '&:last-of-type': { borderBottom: 'none' },
                  '&:hover': { bgcolor: colorCream },
                  '&:hover .doc-actions': { opacity: 1 },
                }}
              >
                <Box
                  className="doc-actions"
                  sx={{ display: 'flex', px: 0.5, opacity: 0.35, transition: 'opacity 0.12s' }}
                >
                  <Tooltip title="Filter for value">
                    <span>
                      <IconButton
                        size="small"
                        aria-label={`Filter for ${field}`}
                        disabled={!filterable}
                        onClick={() => filterable && onFilter(field, value, false)}
                      >
                        <PlusIcon size={13} />
                      </IconButton>
                    </span>
                  </Tooltip>
                  <Tooltip title="Filter out value">
                    <span>
                      <IconButton
                        size="small"
                        aria-label={`Filter out ${field}`}
                        disabled={!filterable}
                        onClick={() => filterable && onFilter(field, value, true)}
                      >
                        <MinusIcon size={13} />
                      </IconButton>
                    </span>
                  </Tooltip>
                  {onToggleColumn ? (
                    <Tooltip title={shown ? 'Remove column' : 'Add as column'}>
                      <IconButton
                        size="small"
                        aria-label={`${shown ? 'Remove' : 'Add'} column ${field}`}
                        onClick={() => onToggleColumn(field)}
                        sx={{ color: shown ? colorBlue : undefined }}
                      >
                        <ColumnIcon size={13} />
                      </IconButton>
                    </Tooltip>
                  ) : null}
                  <Tooltip title="Filter for field present">
                    <IconButton
                      size="small"
                      aria-label={`Filter for ${field} present`}
                      onClick={() => onFilterExists(field)}
                    >
                      <ExistsIcon size={13} />
                    </IconButton>
                  </Tooltip>
                </Box>
                <Box sx={{ ...monoSx, py: 0.75, pr: 1.5, color: colorInk60, overflowWrap: 'anywhere' }}>
                  {field}
                </Box>
                <Box
                  sx={{
                    ...monoSx,
                    py: 0.75,
                    pr: 1.5,
                    color: value === null ? colorInk40 : 'text.primary',
                    overflowWrap: 'anywhere',
                  }}
                >
                  {valueLabel(value)}
                </Box>
              </Box>
            );
          })}
        </Box>
      ) : (
        <Box sx={{ border: HAIRLINE, borderRadius: '4px', overflow: 'hidden' }}>
          <SyntaxHighlighter
            language="json"
            style={oneLight}
            customStyle={{ margin: 0, fontSize: '12px', maxHeight: 420, background: '#FFFFFF' }}
          >
            {json}
          </SyntaxHighlighter>
        </Box>
      )}
    </Box>
  );
}
