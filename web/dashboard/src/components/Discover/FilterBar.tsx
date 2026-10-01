import { useState } from 'react';
import { Box, ButtonBase, Menu, MenuItem } from '@mui/material';
import { Plus as PlusIcon, X as CloseIcon } from 'lucide-react';
import { filterLabel } from '../../discover/filters';
import type { FieldDef, FilterPill } from '../../discover/types';
import {
  colorBlue,
  colorBlueDark,
  colorBluePale,
  colorInk40,
  colorRoseDark,
  colorRosePale,
} from '../../theme/tokens';
import { FilterEditor } from './FilterEditor';
import { monoSx } from './styles';

interface FilterBarProps {
  filters: FilterPill[];
  fields: FieldDef[];
  onChange: (filters: FilterPill[]) => void;
}

/** Which pill's menu or editor is open; `index` -1 is the "add filter" editor. */
interface Target {
  index: number;
  anchor: HTMLElement;
}

export function FilterBar({ filters, fields, onChange }: FilterBarProps) {
  const [menu, setMenu] = useState<Target | null>(null);
  const [editor, setEditor] = useState<Target | null>(null);

  const replace = (index: number, pill: FilterPill) =>
    onChange(filters.map((existing, position) => (position === index ? pill : existing)));
  const remove = (index: number) => onChange(filters.filter((_, position) => position !== index));

  const menuPill = menu ? filters[menu.index] : undefined;

  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 0.75 }}>
      {filters.map((pill, index) => (
        <Box
          key={`${index}:${filterLabel(pill)}`}
          sx={{
            display: 'inline-flex',
            alignItems: 'stretch',
            maxWidth: 420,
            borderRadius: '4px',
            bgcolor: pill.negate ? colorRosePale : colorBluePale,
            color: pill.negate ? colorRoseDark : colorBlueDark,
            opacity: pill.disabled ? 0.55 : 1,
          }}
        >
          <ButtonBase
            onClick={(event) => setMenu({ index, anchor: event.currentTarget })}
            title={pill.disabled ? 'Disabled — click for actions' : 'Click for actions'}
            sx={{
              ...monoSx,
              fontSize: '12px',
              pl: 1,
              pr: 0.5,
              py: 0.375,
              minWidth: 0,
              textDecoration: pill.disabled ? 'line-through' : 'none',
            }}
          >
            {pill.negate ? (
              <Box component="span" sx={{ fontWeight: 700, mr: 0.5 }}>
                NOT
              </Box>
            ) : null}
            <Box
              component="span"
              sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            >
              {filterLabel(pill)}
            </Box>
          </ButtonBase>
          <ButtonBase
            onClick={() => remove(index)}
            aria-label={`Remove filter ${filterLabel(pill)}`}
            sx={{ px: 0.5, borderRadius: '0 4px 4px 0', '&:hover': { bgcolor: 'rgba(24,22,15,0.08)' } }}
          >
            <CloseIcon size={12} />
          </ButtonBase>
        </Box>
      ))}

      <ButtonBase
        onClick={(event) => setEditor({ index: -1, anchor: event.currentTarget })}
        sx={{
          gap: 0.5,
          px: 0.75,
          py: 0.375,
          borderRadius: '4px',
          fontSize: '12px',
          fontWeight: 600,
          color: colorBlue,
          '&:hover': { bgcolor: colorBluePale },
        }}
      >
        <PlusIcon size={13} />
        Add filter
      </ButtonBase>
      {filters.length > 1 ? (
        <ButtonBase
          onClick={() => onChange([])}
          sx={{ px: 0.75, py: 0.375, borderRadius: '4px', fontSize: '12px', color: colorInk40 }}
        >
          Clear all
        </ButtonBase>
      ) : null}

      <Menu anchorEl={menu?.anchor} open={menu !== null} onClose={() => setMenu(null)}>
        {menu && menuPill
          ? [
              <MenuItem
                key="edit"
                sx={{ fontSize: '13px' }}
                onClick={() => {
                  setEditor(menu);
                  setMenu(null);
                }}
              >
                Edit filter
              </MenuItem>,
              <MenuItem
                key="negate"
                sx={{ fontSize: '13px' }}
                onClick={() => {
                  replace(menu.index, { ...menuPill, negate: !menuPill.negate });
                  setMenu(null);
                }}
              >
                {menuPill.negate ? 'Include results' : 'Exclude results'}
              </MenuItem>,
              <MenuItem
                key="disable"
                sx={{ fontSize: '13px' }}
                onClick={() => {
                  replace(menu.index, { ...menuPill, disabled: !menuPill.disabled });
                  setMenu(null);
                }}
              >
                {menuPill.disabled ? 'Re-enable' : 'Temporarily disable'}
              </MenuItem>,
              <MenuItem
                key="delete"
                sx={{ fontSize: '13px' }}
                onClick={() => {
                  remove(menu.index);
                  setMenu(null);
                }}
              >
                Delete
              </MenuItem>,
            ]
          : null}
      </Menu>

      {editor ? (
        <FilterEditor
          // A fresh editor per target, so its form starts from that pill.
          key={editor.index}
          anchor={editor.anchor}
          initial={editor.index >= 0 ? filters[editor.index] : undefined}
          fields={fields}
          onClose={() => setEditor(null)}
          onSave={(pill) => {
            if (editor.index >= 0) {
              replace(editor.index, pill);
            } else {
              onChange([...filters, pill]);
            }
            setEditor(null);
          }}
        />
      ) : null}
    </Box>
  );
}
