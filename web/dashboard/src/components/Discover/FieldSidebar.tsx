import { useMemo, useState } from 'react';
import { Box, ButtonBase, Collapse, InputBase, Tooltip, Typography } from '@mui/material';
import { Search as SearchIcon } from 'lucide-react';
import type { EventDoc, FieldDef, FieldType, FilterValue } from '../../discover/types';
import type { Facets } from '../../services/searchService';
import {
  colorBlue,
  colorBluePale,
  colorCream,
  colorCream2,
  colorInk40,
  colorInk60,
  colorRose,
} from '../../theme/tokens';
import { FieldDetail } from './FieldDetail';
import { HAIRLINE, monoSx, overlineSx, panelSx } from './styles';

const TYPE_BADGE: Record<FieldType, string> = {
  string: 't',
  number: '#',
  boolean: 'b',
  date: 'd',
  ip: 'ip',
  object: '{}',
  unknown: '?',
};

interface FieldSidebarProps {
  /** In pixels. */
  width: number;
  fields: FieldDef[];
  columns: string[];
  docs: EventDoc[];
  searchKey: string | null;
  loadFacets: (field: string, signal: AbortSignal) => Promise<Facets>;
  onToggleColumn: (field: string) => void;
  onFilter: (field: string, value: FilterValue, negate: boolean) => void;
  onFilterExists: (field: string) => void;
}

export function FieldSidebar({
  width,
  fields,
  columns,
  docs,
  searchKey,
  loadFacets,
  onToggleColumn,
  onFilter,
  onFilterExists,
}: FieldSidebarProps) {
  const [search, setSearch] = useState('');
  const [open, setOpen] = useState<string | null>(null);

  const { selected, available } = useMemo(() => {
    const needle = search.trim().toLowerCase();
    const byName = new Map(fields.map((field) => [field.name, field]));
    // A selected column stays listed even before any loaded event has it.
    const chosen = columns.map(
      (name): FieldDef => byName.get(name) ?? { name, type: 'unknown', facetable: false },
    );
    const matches = (field: FieldDef) => field.name.toLowerCase().includes(needle);
    return {
      selected: chosen.filter(matches),
      available: fields.filter((field) => !columns.includes(field.name) && matches(field)),
    };
  }, [fields, columns, search]);

  const renderField = (field: FieldDef, isSelected: boolean) => {
    const expanded = open === field.name;
    return (
      <Box key={field.name} sx={{ borderRadius: '4px', bgcolor: expanded ? colorCream : 'transparent' }}>
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            '&:hover': { bgcolor: colorCream },
            '&:hover .field-toggle, &:focus-within .field-toggle': { opacity: 1 },
            borderRadius: '4px',
          }}
        >
          <ButtonBase
            onClick={() => setOpen(expanded ? null : field.name)}
            aria-expanded={expanded}
            sx={{ flex: 1, minWidth: 0, justifyContent: 'flex-start', gap: 1, px: 1, py: 0.625 }}
          >
            <Tooltip title={field.type} placement="left">
              <Box
                sx={{
                  width: 20,
                  flexShrink: 0,
                  textAlign: 'center',
                  borderRadius: '3px',
                  bgcolor: colorCream2,
                  color: colorInk60,
                  fontSize: '10px',
                  fontWeight: 700,
                  lineHeight: '16px',
                }}
              >
                {TYPE_BADGE[field.type]}
              </Box>
            </Tooltip>
            <Box
              title={field.name}
              sx={{
                ...monoSx,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
                color: 'text.primary',
              }}
            >
              {field.name}
            </Box>
          </ButtonBase>
          <ButtonBase
            className="field-toggle"
            onClick={() => onToggleColumn(field.name)}
            aria-label={`${isSelected ? 'Remove' : 'Add'} ${field.name} ${isSelected ? 'from' : 'to'} the table`}
            sx={{
              opacity: 0,
              mr: 0.5,
              px: 0.75,
              py: 0.25,
              borderRadius: '3px',
              fontSize: '11px',
              fontWeight: 600,
              color: isSelected ? colorRose : colorBlue,
              bgcolor: isSelected ? 'transparent' : colorBluePale,
            }}
          >
            {isSelected ? 'remove' : 'add'}
          </ButtonBase>
        </Box>
        <Collapse in={expanded} unmountOnExit>
          <FieldDetail
            field={field}
            docs={docs}
            searchKey={searchKey}
            loadFacets={loadFacets}
            onFilter={onFilter}
            onFilterExists={onFilterExists}
          />
        </Collapse>
      </Box>
    );
  };

  return (
    <Box
      component="aside"
      sx={{ ...panelSx, width, flexShrink: 0, display: 'flex', flexDirection: 'column', minHeight: 0 }}
    >
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, px: 1.5, py: 1, borderBottom: HAIRLINE }}>
        <SearchIcon size={14} color={colorInk40} />
        <InputBase
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search field names"
          inputProps={{ 'aria-label': 'Search field names' }}
          sx={{ flex: 1, fontSize: '13px' }}
        />
      </Box>

      <Box sx={{ flex: 1, overflowY: 'auto', p: 1 }}>
        {selected.length > 0 ? (
          <>
            <Typography sx={{ ...overlineSx, px: 1, py: 0.5 }}>Selected fields</Typography>
            {selected.map((field) => renderField(field, true))}
          </>
        ) : null}

        <Typography sx={{ ...overlineSx, px: 1, py: 0.5, mt: selected.length > 0 ? 1 : 0 }}>
          Available fields
        </Typography>
        {available.map((field) => renderField(field, false))}
        {available.length === 0 ? (
          <Typography sx={{ px: 1, py: 0.5, fontSize: '12px', color: colorInk40 }}>
            No fields match.
          </Typography>
        ) : null}
      </Box>
    </Box>
  );
}
