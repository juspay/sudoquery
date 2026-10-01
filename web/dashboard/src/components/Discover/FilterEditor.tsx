import { useState } from 'react';
import { Autocomplete, Box, Button, MenuItem, Popover, TextField, Typography } from '@mui/material';
import type { FieldDef, FilterPill, FilterValue } from '../../discover/types';
import { HAIRLINE, monoSx, overlineSx } from './styles';

type Choice = 'is' | 'is_not' | 'one_of' | 'not_one_of' | 'exists' | 'not_exists' | 'between' | 'not_between';

const CHOICES: Array<{ id: Choice; label: string; operator: FilterPill['operator']; negate: boolean }> = [
  { id: 'is', label: 'is', operator: 'is', negate: false },
  { id: 'is_not', label: 'is not', operator: 'is', negate: true },
  { id: 'one_of', label: 'is one of', operator: 'is_one_of', negate: false },
  { id: 'not_one_of', label: 'is not one of', operator: 'is_one_of', negate: true },
  { id: 'exists', label: 'exists', operator: 'exists', negate: false },
  { id: 'not_exists', label: 'does not exist', operator: 'exists', negate: true },
  { id: 'between', label: 'is between', operator: 'range', negate: false },
  { id: 'not_between', label: 'is not between', operator: 'range', negate: true },
];

function choiceOf(pill: FilterPill): Choice {
  const match = CHOICES.find(
    (choice) => choice.operator === pill.operator && choice.negate === pill.negate,
  );
  return match?.id ?? 'is';
}

/** Reads typed text as the field's type: numbers and booleans are not sent as strings. */
function typed(text: string, field: FieldDef | undefined): FilterValue {
  const trimmed = text.trim();
  if (field?.type === 'number' && trimmed !== '' && Number.isFinite(Number(trimmed))) {
    return Number(trimmed);
  }
  if (field?.type === 'boolean' && (trimmed === 'true' || trimmed === 'false')) {
    return trimmed === 'true';
  }
  return trimmed;
}

const text = (value: FilterValue | undefined) => (value === undefined ? '' : String(value));

interface FilterEditorProps {
  anchor: HTMLElement | null;
  /** The pill being edited; omit to create one. */
  initial?: FilterPill;
  fields: FieldDef[];
  onClose: () => void;
  onSave: (pill: FilterPill) => void;
}

export function FilterEditor({ anchor, initial, fields, onClose, onSave }: FilterEditorProps) {
  const [field, setField] = useState(initial?.field ?? '');
  const [choice, setChoice] = useState<Choice>(initial ? choiceOf(initial) : 'is');
  const [value, setValue] = useState(text(initial?.value));
  const [list, setList] = useState((initial?.values ?? []).map(String).join(', '));
  const [gte, setGte] = useState(text(initial?.gte));
  const [lt, setLt] = useState(text(initial?.lt));

  const selected = CHOICES.find((candidate) => candidate.id === choice) ?? CHOICES[0];
  const fieldDef = fields.find((candidate) => candidate.name === field.trim());

  const build = (): FilterPill | null => {
    const name = field.trim();
    if (name === '') return null;
    const base = {
      field: name,
      operator: selected.operator,
      negate: selected.negate,
      disabled: initial?.disabled ?? false,
    };
    switch (selected.operator) {
      case 'is':
        return value.trim() === '' ? null : { ...base, value: typed(value, fieldDef) };
      case 'is_one_of': {
        const values = list
          .split(',')
          .filter((entry) => entry.trim() !== '')
          .map((entry) => typed(entry, fieldDef));
        return values.length === 0 ? null : { ...base, values };
      }
      case 'exists':
        return base;
      case 'range': {
        if (gte.trim() === '' && lt.trim() === '') return null;
        return {
          ...base,
          ...(gte.trim() === '' ? {} : { gte: typed(gte, fieldDef) }),
          ...(lt.trim() === '' ? {} : { lt: typed(lt, fieldDef) }),
        };
      }
    }
  };

  const pill = build();

  return (
    <Popover
      open={anchor !== null}
      anchorEl={anchor}
      onClose={onClose}
      anchorOrigin={{ vertical: 'bottom', horizontal: 'left' }}
      slotProps={{ paper: { sx: { mt: 0.5, p: 2, width: 420, border: HAIRLINE } } }}
    >
      <Box
        component="form"
        onSubmit={(event: React.FormEvent) => {
          event.preventDefault();
          if (pill) onSave(pill);
        }}
        sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}
      >
        <Typography sx={overlineSx}>{initial ? 'Edit filter' : 'Add filter'}</Typography>
        <Box sx={{ display: 'flex', gap: 1 }}>
          <Autocomplete
            freeSolo
            size="small"
            options={fields.map((candidate) => candidate.name)}
            inputValue={field}
            onInputChange={(_, next) => setField(next)}
            sx={{ flex: 1 }}
            slotProps={{ paper: { sx: { '& .MuiAutocomplete-option': monoSx } } }}
            renderInput={(params) => (
              <TextField {...params} label="Field" autoFocus={!initial} />
            )}
          />
          <TextField
            select
            size="small"
            label="Operator"
            value={choice}
            onChange={(event) => setChoice(event.target.value as Choice)}
            sx={{ width: 160 }}
          >
            {CHOICES.map((option) => (
              <MenuItem key={option.id} value={option.id} sx={{ fontSize: '13px' }}>
                {option.label}
              </MenuItem>
            ))}
          </TextField>
        </Box>

        {selected.operator === 'is' ? (
          <TextField
            size="small"
            label="Value"
            value={value}
            onChange={(event) => setValue(event.target.value)}
          />
        ) : null}
        {selected.operator === 'is_one_of' ? (
          <TextField
            size="small"
            label="Values"
            helperText="Separate values with commas"
            value={list}
            onChange={(event) => setList(event.target.value)}
          />
        ) : null}
        {selected.operator === 'range' ? (
          <Box sx={{ display: 'flex', gap: 1 }}>
            <TextField
              size="small"
              label="From (inclusive)"
              value={gte}
              onChange={(event) => setGte(event.target.value)}
              sx={{ flex: 1 }}
            />
            <TextField
              size="small"
              label="To (exclusive)"
              value={lt}
              onChange={(event) => setLt(event.target.value)}
              sx={{ flex: 1 }}
            />
          </Box>
        ) : null}

        <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1 }}>
          <Button size="small" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" size="small" variant="contained" disabled={!pill}>
            Save
          </Button>
        </Box>
      </Box>
    </Popover>
  );
}
