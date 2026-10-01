import { useMemo } from 'react';
import { Box, Tooltip, Typography } from '@mui/material';
import { formatCount, formatOffset } from '../../discover/format';
import { colorCream, colorInk40, colorInk60 } from '../../theme/tokens';
import { HAIRLINE, monoSx } from '../Discover/styles';

const FALLBACK_COLOR = colorInk60;

export interface SessionEvent {
  /** Position in the session's event list. */
  index: number;
  name: string;
  /** Milliseconds since the epoch. */
  time: number;
}

interface SessionStripProps {
  events: SessionEvent[];
  /** Colour per event name. */
  colors: Map<string, string>;
  onSelect: (index: number) => void;
}

/**
 * Where a session's events fall between its first and last event: one tick
 * per event, coloured by event name, with the names and their counts below.
 */
export function SessionStrip({ events, colors, onSelect }: SessionStripProps) {
  const start = events[0]?.time ?? 0;
  const span = Math.max(1, (events[events.length - 1]?.time ?? 0) - start);

  const names = useMemo(() => {
    const counts = new Map<string, number>();
    for (const event of events) counts.set(event.name, (counts.get(event.name) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  }, [events]);

  return (
    <Box>
      <Box
        sx={{
          position: 'relative',
          height: 36,
          borderRadius: '4px',
          bgcolor: colorCream,
          border: HAIRLINE,
          overflow: 'hidden',
        }}
      >
        {events.map((event) => (
          <Tooltip
            key={event.index}
            title={`${event.name} · ${formatOffset(event.time - start)}`}
            placement="top"
            arrow
          >
            <Box
              component="button"
              type="button"
              aria-label={`${event.name} at ${formatOffset(event.time - start)}`}
              onClick={() => onSelect(event.index)}
              sx={{
                position: 'absolute',
                top: 4,
                bottom: 4,
                // Keeps the first and last tick inside the strip.
                left: `calc(${((event.time - start) / span) * 100}% * 0.98 + 1%)`,
                width: 4,
                ml: '-2px',
                p: 0,
                border: 'none',
                borderRadius: '2px',
                bgcolor: colors.get(event.name) ?? FALLBACK_COLOR,
                opacity: 0.85,
                cursor: 'pointer',
                '&:hover, &:focus-visible': { opacity: 1, width: 6, ml: '-3px', outline: 'none' },
              }}
            />
          </Tooltip>
        ))}
      </Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', mt: 0.25 }}>
        <Typography sx={{ fontSize: '10.5px', color: colorInk40 }}>start</Typography>
        <Typography sx={{ fontSize: '10.5px', color: colorInk40 }}>{formatOffset(span)}</Typography>
      </Box>

      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1.5, mt: 0.75 }}>
        {names.map(([name, count]) => (
          <Box key={name} sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
            <Box
              sx={{ width: 8, height: 8, borderRadius: '2px', bgcolor: colors.get(name) ?? FALLBACK_COLOR }}
            />
            <Typography sx={{ ...monoSx, fontSize: '11.5px', color: 'text.primary' }}>{name}</Typography>
            <Typography sx={{ fontSize: '11px', color: colorInk60 }}>{formatCount(count)}</Typography>
          </Box>
        ))}
      </Box>
    </Box>
  );
}
