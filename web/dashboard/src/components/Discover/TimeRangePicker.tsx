import { useState } from 'react';
import {
  Box,
  Button,
  ButtonBase,
  Checkbox,
  FormControlLabel,
  Popover,
  TextField,
  Typography,
} from '@mui/material';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { DateTimePicker } from '@mui/x-date-pickers/DateTimePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { Clock as ClockIcon } from 'lucide-react';
import { QUICK_RANGES, rangeLabel, resolveRange } from '../../discover/timeRange';
import type { TimeRange } from '../../discover/types';
import {
  colorBlue,
  colorBluePale,
  colorCream,
  colorCream2,
  colorInk,
  colorInk60,
  colorRose,
} from '../../theme/tokens';
import { HAIRLINE, overlineSx } from './styles';

interface TimeRangePickerProps {
  value: TimeRange;
  onChange: (range: TimeRange) => void;
}

interface Draft {
  from: Date | null;
  to: Date | null;
  /** The range ends at "now", which moves forward on every refresh. */
  toNow: boolean;
  /** When the picker was opened; "now" for checking the draft. */
  openedAt: Date;
}

const pickerSlotProps = {
  textField: {
    size: 'small' as const,
    fullWidth: true,
    sx: { '& .MuiInputBase-input': { fontSize: '13px' } },
  },
};

export function TimeRangePicker({ value, onChange }: TimeRangePickerProps) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [draft, setDraft] = useState<Draft>({
    from: null,
    to: null,
    toNow: false,
    openedAt: new Date(0),
  });

  const open = (event: React.MouseEvent<HTMLElement>) => {
    // Start the absolute pickers at what the current range means right now.
    const openedAt = new Date();
    const resolved = resolveRange(value, openedAt);
    setDraft({
      from: resolved?.from ?? null,
      to: resolved?.to ?? null,
      toNow: value.to.trim() === 'now',
      openedAt,
    });
    setAnchor(event.currentTarget);
  };

  const choose = (range: TimeRange) => {
    setAnchor(null);
    onChange(range);
  };

  const end = draft.toNow ? draft.openedAt : draft.to;
  const valid = draft.from !== null && end !== null && draft.from < end;

  return (
    <>
      <ButtonBase
        onClick={open}
        aria-label="Time range"
        sx={{
          height: 40,
          px: 1.5,
          gap: 1,
          flexShrink: 0,
          bgcolor: '#FFFFFF',
          border: `1.5px solid ${colorCream2}`,
          borderRadius: '5px',
          fontSize: '13px',
          fontWeight: 600,
          color: colorInk,
          whiteSpace: 'nowrap',
          '&:hover': { bgcolor: colorCream },
        }}
      >
        <ClockIcon size={15} color={colorBlue} />
        {rangeLabel(value)}
      </ButtonBase>

      <Popover
        open={anchor !== null}
        anchorEl={anchor}
        onClose={() => setAnchor(null)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
        transformOrigin={{ vertical: 'top', horizontal: 'right' }}
        slotProps={{ paper: { sx: { mt: 0.5, border: HAIRLINE, display: 'flex' } } }}
      >
        <Box sx={{ p: 1.5, width: 190, borderRight: HAIRLINE }}>
          <Typography sx={{ ...overlineSx, px: 1, mb: 0.5 }}>Quick ranges</Typography>
          {QUICK_RANGES.map(({ label, range }) => {
            const selected = range.from === value.from && range.to === value.to;
            return (
              <ButtonBase
                key={label}
                onClick={() => choose(range)}
                sx={{
                  display: 'block',
                  width: '100%',
                  textAlign: 'left',
                  px: 1,
                  py: 0.75,
                  borderRadius: '4px',
                  fontSize: '13px',
                  fontWeight: selected ? 600 : 500,
                  color: selected ? colorBlue : colorInk,
                  bgcolor: selected ? colorBluePale : 'transparent',
                  '&:hover': { bgcolor: selected ? colorBluePale : colorCream },
                }}
              >
                {label}
              </ButtonBase>
            );
          })}
        </Box>

        <Box sx={{ p: 2, width: 280, display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          <Typography sx={overlineSx}>Absolute range</Typography>
          <LocalizationProvider dateAdapter={AdapterDateFns}>
            <DateTimePicker
              label="From"
              value={draft.from}
              onChange={(from) => setDraft((current) => ({ ...current, from }))}
              views={['year', 'month', 'day', 'hours', 'minutes', 'seconds']}
              ampm={false}
              slotProps={pickerSlotProps}
            />
            {draft.toNow ? (
              <TextField
                label="To"
                value="Now"
                size="small"
                fullWidth
                disabled
                helperText="Moves forward with every refresh"
                sx={{ '& .MuiInputBase-input': { fontSize: '13px' } }}
              />
            ) : (
              <DateTimePicker
                label="To"
                value={draft.to}
                onChange={(to) => setDraft((current) => ({ ...current, to }))}
                views={['year', 'month', 'day', 'hours', 'minutes', 'seconds']}
                ampm={false}
                slotProps={pickerSlotProps}
              />
            )}
          </LocalizationProvider>
          <FormControlLabel
            label="End at now"
            control={
              <Checkbox
                size="small"
                checked={draft.toNow}
                onChange={(event) => {
                  const toNow = event.target.checked;
                  setDraft((current) => ({ ...current, toNow, to: current.to ?? current.openedAt }));
                }}
              />
            }
            sx={{ mt: -1, '& .MuiFormControlLabel-label': { fontSize: '13px', color: colorInk60 } }}
          />
          {!valid && draft.from !== null && end !== null ? (
            <Typography sx={{ fontSize: '12px', color: colorRose }}>
              "From" must be before "To".
            </Typography>
          ) : null}
          <Button
            variant="contained"
            size="small"
            disabled={!valid}
            onClick={() => {
              const to = draft.toNow ? 'now' : draft.to?.toISOString();
              if (draft.from && to) {
                choose({ from: draft.from.toISOString(), to });
              }
            }}
            sx={{ alignSelf: 'flex-end' }}
          >
            Apply
          </Button>
        </Box>
      </Popover>
    </>
  );
}
