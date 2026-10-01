import { useState } from 'react';
import { Box, ButtonBase, Menu, MenuItem, Tooltip } from '@mui/material';
import { Check as CheckIcon, ChevronDown as ChevronDownIcon, Timer as TimerIcon } from 'lucide-react';
import { REFRESH_INTERVALS } from '../../discover/autoRefresh';
import {
  colorBlue,
  colorBluePale,
  colorCream,
  colorCream2,
  colorInk60,
  colorRose,
} from '../../theme/tokens';

interface AutoRefreshPickerProps {
  /** `null` when auto-refresh is off. */
  value: string | null;
  /** The time range ends at a fixed moment, so refreshing shows nothing new. */
  fixedEnd: boolean;
  onChange: (value: string | null) => void;
}

export function AutoRefreshPicker({ value, fixedEnd, onChange }: AutoRefreshPickerProps) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const active = value !== null;
  const label = REFRESH_INTERVALS.find((option) => option.value === value)?.label;

  const choose = (next: string | null) => {
    setAnchor(null);
    onChange(next);
  };

  return (
    <>
      <Tooltip
        title={
          !active
            ? 'Auto-refresh is off'
            : fixedEnd
              ? `Refreshing every ${label}, but the time range has a fixed end, so new events will not appear. Set its end to Now.`
              : `Refreshing every ${label}`
        }
      >
        <ButtonBase
          onClick={(event) => setAnchor(event.currentTarget)}
          aria-label="Auto-refresh"
          aria-haspopup="menu"
          sx={{
            height: 40,
            px: 1.25,
            gap: 0.5,
            flexShrink: 0,
            borderRadius: '5px',
            border: `1.5px solid ${active ? colorBluePale : colorCream2}`,
            bgcolor: active ? colorBluePale : '#FFFFFF',
            color: active ? colorBlue : colorInk60,
            fontSize: '13px',
            fontWeight: 600,
            whiteSpace: 'nowrap',
            '&:hover': { bgcolor: active ? colorBluePale : colorCream },
          }}
        >
          <TimerIcon size={15} />
          {active ? value : 'Off'}
          {active && fixedEnd ? (
            <Box
              component="span"
              aria-label="fixed end"
              sx={{ width: 6, height: 6, borderRadius: '50%', bgcolor: colorRose }}
            />
          ) : null}
          <ChevronDownIcon size={13} />
        </ButtonBase>
      </Tooltip>

      <Menu
        anchorEl={anchor}
        open={anchor !== null}
        onClose={() => setAnchor(null)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
        transformOrigin={{ vertical: 'top', horizontal: 'right' }}
        slotProps={{ list: { dense: true, 'aria-label': 'Refresh every' } }}
      >
        {[{ value: null, label: 'Off' }, ...REFRESH_INTERVALS].map((option) => {
          const selected = option.value === value;
          return (
            <MenuItem
              key={option.value ?? 'off'}
              selected={selected}
              onClick={() => choose(option.value)}
              sx={{ fontSize: '13px', gap: 1, minWidth: 160 }}
            >
              <CheckIcon size={14} style={{ visibility: selected ? 'visible' : 'hidden' }} />
              {option.value === null ? 'Off' : `Every ${option.label}`}
            </MenuItem>
          );
        })}
      </Menu>
    </>
  );
}
