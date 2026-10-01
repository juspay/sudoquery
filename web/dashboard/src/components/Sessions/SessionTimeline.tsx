import { useMemo } from 'react';
import { format } from 'date-fns';
import { Box, ButtonBase, Typography } from '@mui/material';
import { ChevronDown as CollapseIcon, ChevronRight as ExpandIcon } from 'lucide-react';
import { flattenEvent } from '../../discover/fields';
import { eventElementId, formatDuration, formatOffset } from '../../discover/format';
import { valueLabel } from '../../discover/topValues';
import type { EventDoc } from '../../discover/types';
import { colorCream, colorInk20, colorInk40, colorInk60 } from '../../theme/tokens';
import { DocDetail, type DocActions } from '../Discover/DocDetail';
import { HAIRLINE, monoSx } from '../Discover/styles';

/** A pause at least this long between two events is called out. */
const IDLE_GAP_MS = 30_000;
/** Properties shown next to the event name before expanding it. */
const MAX_INLINE_PROPERTIES = 4;

const GUTTER = 148;

function eventName(doc: EventDoc): string {
  return typeof doc.name === 'string' ? doc.name : '(unnamed)';
}

function inlineProperties(doc: EventDoc): Array<[string, unknown]> {
  return Object.entries(flattenEvent(doc))
    .filter(([field]) => field.startsWith('properties.'))
    .slice(0, MAX_INLINE_PROPERTIES)
    .map(([field, value]) => [field.slice('properties.'.length), value]);
}

interface TimelineEventProps extends DocActions {
  doc: EventDoc;
  color: string;
  index: number;
  time: number;
  start: number;
  /** Milliseconds since the previous event; `null` for the first. */
  gap: number | null;
  last: boolean;
  expanded: boolean;
  onToggle: () => void;
}

function TimelineEvent({
  doc,
  color,
  index,
  time,
  start,
  gap,
  last,
  expanded,
  onToggle,
  ...actions
}: TimelineEventProps) {
  const name = eventName(doc);
  const properties = useMemo(() => inlineProperties(doc), [doc]);

  return (
    <Box id={eventElementId(index)} sx={{ scrollMarginTop: 12 }}>
      {gap !== null && gap >= IDLE_GAP_MS ? (
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Box sx={{ width: GUTTER, flexShrink: 0 }} />
          <Box sx={{ width: 20, display: 'flex', justifyContent: 'center', alignSelf: 'stretch' }}>
            <Box sx={{ borderLeft: `2px dotted ${colorInk20}` }} />
          </Box>
          <Typography sx={{ py: 1, pl: 1, fontSize: '11.5px', fontStyle: 'italic', color: colorInk40 }}>
            {formatDuration(gap)} with no events
          </Typography>
        </Box>
      ) : null}

      <Box sx={{ display: 'flex', alignItems: 'stretch' }}>
        <Box sx={{ width: GUTTER, flexShrink: 0, pt: 0.875, pr: 1.5, textAlign: 'right' }}>
          <Typography sx={{ ...monoSx, fontSize: '12px', color: 'text.primary' }}>
            {format(new Date(time), 'HH:mm:ss.SSS')}
          </Typography>
          <Typography sx={{ ...monoSx, fontSize: '11px', color: colorInk40 }}>
            {formatOffset(time - start)}
          </Typography>
        </Box>

        {/* The rail: a dot per event on a line that stops at the last one. */}
        <Box sx={{ width: 20, flexShrink: 0, position: 'relative' }}>
          <Box
            sx={{
              position: 'absolute',
              left: '50%',
              top: index === 0 ? 14 : 0,
              bottom: last ? 'calc(100% - 14px)' : 0,
              ml: '-1px',
              borderLeft: `2px solid ${colorInk20}`,
            }}
          />
          <Box
            sx={{
              position: 'absolute',
              left: '50%',
              top: 9,
              width: 10,
              height: 10,
              ml: '-5px',
              borderRadius: '50%',
              bgcolor: color,
              border: '2px solid #FFFFFF',
              boxShadow: `0 0 0 1px ${color}`,
            }}
          />
        </Box>

        <Box sx={{ flex: 1, minWidth: 0, pb: 0.75, pl: 1 }}>
          <Box
            sx={{
              border: HAIRLINE,
              borderRadius: '4px',
              bgcolor: '#FFFFFF',
              overflow: 'hidden',
            }}
          >
            <ButtonBase
              onClick={onToggle}
              aria-expanded={expanded}
              sx={{
                width: '100%',
                justifyContent: 'flex-start',
                alignItems: 'center',
                gap: 1,
                px: 1,
                py: 0.75,
                textAlign: 'left',
                '&:hover': { bgcolor: colorCream },
              }}
            >
              <Box sx={{ color: colorInk60, lineHeight: 0, flexShrink: 0 }}>
                {expanded ? <CollapseIcon size={14} /> : <ExpandIcon size={14} />}
              </Box>
              <Box
                sx={{
                  ...monoSx,
                  flexShrink: 0,
                  px: 0.75,
                  borderRadius: '3px',
                  fontWeight: 600,
                  color,
                  bgcolor: `${color}1A`,
                }}
              >
                {name}
              </Box>
              <Box
                sx={{
                  ...monoSx,
                  fontSize: '12px',
                  minWidth: 0,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  color: colorInk60,
                }}
              >
                {properties.map(([key, value]) => (
                  <Box component="span" key={key} sx={{ mr: 1.5 }}>
                    {key}=
                    <Box component="span" sx={{ color: 'text.primary' }}>
                      {valueLabel(value)}
                    </Box>
                  </Box>
                ))}
              </Box>
            </ButtonBase>
            {expanded ? <DocDetail doc={doc} {...actions} /> : null}
          </Box>
        </Box>
      </Box>
    </Box>
  );
}

interface SessionTimelineProps extends DocActions {
  events: EventDoc[];
  /** Colour per event name. */
  colors: Map<string, string>;
  /** Event times in milliseconds since the epoch, parallel to `events`. */
  times: number[];
  expanded: ReadonlySet<number>;
  onToggle: (index: number) => void;
}

/** A session's events, oldest first, on a vertical time rail. */
export function SessionTimeline({
  events,
  colors,
  times,
  expanded,
  onToggle,
  ...actions
}: SessionTimelineProps) {
  const start = times[0] ?? 0;
  return (
    <Box>
      {events.map((doc, index) => (
        <TimelineEvent
          key={typeof doc.id === 'string' ? doc.id : index}
          doc={doc}
          color={colors.get(eventName(doc)) ?? colorInk60}
          index={index}
          time={times[index]}
          start={start}
          gap={index === 0 ? null : times[index] - times[index - 1]}
          last={index === events.length - 1}
          expanded={expanded.has(index)}
          onToggle={() => onToggle(index)}
          {...actions}
        />
      ))}
    </Box>
  );
}
