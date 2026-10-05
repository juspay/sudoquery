import { createContext, useContext, useMemo, useState } from 'react';
import { Box, Typography } from '@mui/material';
import { ResponsiveBar, type BarCustomLayerProps, type BarLayer } from '@nivo/bar';
import { formatCount } from '../../discover/format';
import { bucketSelection, nearestBar, type Bucket } from '../../discover/histogram';
import { formatBucket, formatTimestamp, intervalMs } from '../../discover/timeRange';
import type { Histogram as HistogramData } from '../../services/searchService';
import { colorBlue, colorInk, colorInk40, fontFamilyBody } from '../../theme/tokens';
import { getAxisColor, getTooltipBackground, getTooltipTextColor } from '../../utils/chartTheme';

/** Roughly how many labels fit under the bars. */
const MAX_TICKS = 8;
const TOOLTIP_WIDTH = 200;
const TOOLTIP_HEIGHT = 64;

type Datum = { time: string; count: number };

interface Brush {
  buckets: Bucket[];
  interval: string;
  hover: number | null;
  drag: { anchor: number; current: number } | null;
  selectable: boolean;
  setHover: (index: number | null) => void;
  start: (index: number) => void;
  move: (index: number) => void;
  finish: () => void;
  cancel: () => void;
}

const BrushContext = createContext<Brush | null>(null);

function Tooltip({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        display: 'inline-block',
        padding: '6px 10px',
        background: getTooltipBackground(),
        color: getTooltipTextColor(),
        borderRadius: 4,
        fontSize: 12,
        lineHeight: 1.45,
        fontFamily: fontFamilyBody,
        boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
        whiteSpace: 'nowrap',
      }}
    >
      {children}
    </div>
  );
}

/**
 * Drawn over the bars: the hover column, the selection while dragging, the
 * tooltip, and a transparent surface that turns pointer moves into bucket
 * indices. It is a component Nivo renders, so it reads its state from
 * context rather than closing over the chart's props.
 */
function BrushLayer({ bars, innerWidth, innerHeight }: BarCustomLayerProps<Datum>) {
  const brush = useContext(BrushContext);
  if (!brush) return null;

  const spans = bars.map((bar) => ({ index: bar.data.index, x: bar.x, width: bar.width }));
  const byIndex = new Map(spans.map((span) => [span.index, span]));
  // Half the gap between bars, so highlights cover whole columns.
  const pad = spans.length > 1 ? Math.max(0, (spans[1].x - spans[0].x - spans[0].width) / 2) : 0;

  const indexAt = (event: React.PointerEvent<SVGRectElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    return nearestBar(spans, event.clientX - box.left);
  };

  const column = (from: number, to: number) => {
    const first = byIndex.get(Math.min(from, to));
    const last = byIndex.get(Math.max(from, to));
    if (!first || !last) return null;
    const x = Math.max(0, first.x - pad);
    return { x, width: Math.min(innerWidth, last.x + last.width + pad) - x, center: (first.x + last.x + last.width) / 2 };
  };

  const { drag, hover } = brush;
  const selected = drag ? column(drag.anchor, drag.current) : null;
  const hovered = !drag && hover !== null ? column(hover, hover) : null;
  const selection = drag ? bucketSelection(brush.buckets, brush.interval, drag.anchor, drag.current) : null;
  const hoveredBucket = hover !== null ? brush.buckets[hover] : undefined;
  const tooltipAt = selected ?? hovered;

  return (
    <g>
      {hovered ? (
        <rect x={hovered.x} y={0} width={hovered.width} height={innerHeight} fill={colorInk} opacity={0.05} />
      ) : null}
      {selected ? (
        <rect
          x={selected.x}
          y={0}
          width={selected.width}
          height={innerHeight}
          fill={colorBlue}
          fillOpacity={0.14}
          stroke={colorBlue}
          strokeOpacity={0.6}
        />
      ) : null}

      {tooltipAt ? (
        <foreignObject
          x={Math.min(Math.max(0, tooltipAt.center - TOOLTIP_WIDTH / 2), Math.max(0, innerWidth - TOOLTIP_WIDTH))}
          y={0}
          width={TOOLTIP_WIDTH}
          height={TOOLTIP_HEIGHT}
          style={{ pointerEvents: 'none', overflow: 'visible', textAlign: 'center' }}
        >
          {selection ? (
            <Tooltip>
              <div style={{ fontWeight: 600 }}>
                {formatCount(selection.count)} event{selection.count === 1 ? '' : 's'}
              </div>
              <div>{formatTimestamp(selection.from)}</div>
              <div>→ {formatTimestamp(selection.to)}</div>
            </Tooltip>
          ) : hoveredBucket ? (
            <Tooltip>
              <div style={{ fontWeight: 600 }}>
                {formatCount(hoveredBucket.count)} event{hoveredBucket.count === 1 ? '' : 's'}
              </div>
              <div>{formatTimestamp(new Date(hoveredBucket.time))}</div>
              <div style={{ color: colorInk40 }}>per {brush.interval}</div>
            </Tooltip>
          ) : null}
        </foreignObject>
      ) : null}

      <rect
        x={0}
        y={0}
        width={innerWidth}
        height={innerHeight}
        fill="transparent"
        style={{ cursor: brush.selectable ? 'crosshair' : 'default', touchAction: 'none' }}
        onPointerDown={(event) => {
          if (!brush.selectable || event.button !== 0) return;
          const index = indexAt(event);
          if (index === null) return;
          event.currentTarget.setPointerCapture(event.pointerId);
          brush.start(index);
        }}
        onPointerMove={(event) => {
          const index = indexAt(event);
          if (index === null) return;
          if (drag) {
            brush.move(index);
          } else {
            brush.setHover(index);
          }
        }}
        onPointerUp={(event) => {
          if (!drag) return;
          event.currentTarget.releasePointerCapture(event.pointerId);
          brush.finish();
        }}
        onPointerCancel={brush.cancel}
        onPointerLeave={() => {
          if (!drag) brush.setHover(null);
        }}
      />
    </g>
  );
}

const LAYERS: BarLayer<Datum>[] = ['grid', 'axes', 'bars', BrushLayer];

interface HistogramProps {
  data: HistogramData;
  height?: number;
  /** Narrows the search to the buckets clicked or dragged across. */
  onSelect?: (from: Date, to: Date) => void;
}

export function Histogram({ data, height = 132, onSelect }: HistogramProps) {
  const [hover, setHover] = useState<number | null>(null);
  const [drag, setDrag] = useState<{ anchor: number; current: number } | null>(null);

  const width = intervalMs(data.interval) ?? 0;
  const first = data.buckets[0]?.time;
  const last = data.buckets[data.buckets.length - 1]?.time;
  const span = first && last ? new Date(last).getTime() - new Date(first).getTime() + width : 0;

  const bars = useMemo(
    () => data.buckets.map((bucket) => ({ time: bucket.time, count: bucket.count })),
    [data.buckets],
  );
  const ticks = useMemo(() => {
    const step = Math.max(1, Math.ceil(bars.length / MAX_TICKS));
    return bars.filter((_, index) => index % step === 0).map((bar) => bar.time);
  }, [bars]);

  const brush: Brush = {
    buckets: data.buckets,
    interval: data.interval,
    hover,
    drag,
    selectable: onSelect !== undefined,
    setHover,
    start: (index) => setDrag({ anchor: index, current: index }),
    move: (index) => {
      setHover(index);
      setDrag((current) => (current ? { ...current, current: index } : current));
    },
    finish: () => {
      if (drag) {
        const selection = bucketSelection(data.buckets, data.interval, drag.anchor, drag.current);
        if (selection) onSelect?.(selection.from, selection.to);
      }
      setDrag(null);
      setHover(null);
    },
    cancel: () => setDrag(null),
  };

  if (bars.length === 0) {
    return (
      <Box sx={{ height, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Typography sx={{ fontSize: '12px', color: colorInk40 }}>
          No events in this time range
        </Typography>
      </Box>
    );
  }

  return (
    <Box sx={{ height, userSelect: 'none' }}>
      <BrushContext.Provider value={brush}>
        <ResponsiveBar
          data={bars}
          keys={['count']}
          indexBy="time"
          margin={{ top: 8, right: 8, bottom: 24, left: 44 }}
          padding={0.15}
          colors={[colorBlue]}
          borderRadius={1}
          enableLabel={false}
          enableGridX={false}
          gridYValues={3}
          axisLeft={{ tickSize: 0, tickPadding: 6, tickValues: 3, format: (value) => formatCount(Number(value)) }}
          axisBottom={{
            tickSize: 0,
            tickPadding: 6,
            tickValues: ticks,
            format: (value) => formatBucket(new Date(String(value)), data.interval, span),
          }}
          // The brush layer handles hover and clicks for the whole plot.
          isInteractive={false}
          layers={LAYERS}
          theme={{
            axis: { ticks: { text: { fill: colorInk40, fontFamily: fontFamilyBody, fontSize: 11 } } },
            grid: { line: { stroke: getAxisColor(), strokeDasharray: '4 4' } },
          }}
          animate={false}
          role="img"
          ariaLabel="Events over time — click a bar or drag across bars to narrow the time range"
        />
      </BrushContext.Provider>
    </Box>
  );
}
