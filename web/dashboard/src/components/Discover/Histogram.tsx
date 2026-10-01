import { useMemo } from 'react';
import { Box, Typography } from '@mui/material';
import { ResponsiveBar } from '@nivo/bar';
import { formatCount } from '../../discover/format';
import { formatBucket, formatTimestamp, intervalMs } from '../../discover/timeRange';
import type { Histogram as HistogramData } from '../../services/searchService';
import { colorBlue, colorInk40, fontFamilyBody } from '../../theme/tokens';
import { getAxisColor, getTooltipBackground, getTooltipTextColor } from '../../utils/chartTheme';

/** Roughly how many labels fit under the bars. */
const MAX_TICKS = 8;

interface HistogramProps {
  data: HistogramData;
  height?: number;
  /** Narrows the search to one bucket. */
  onZoom?: (from: Date, to: Date) => void;
}

export function Histogram({ data, height = 132, onZoom }: HistogramProps) {
  const width = intervalMs(data.interval) ?? 0;

  const bars = useMemo(
    () => data.buckets.map((bucket) => ({ time: bucket.time, count: bucket.count })),
    [data.buckets],
  );
  const ticks = useMemo(() => {
    const step = Math.max(1, Math.ceil(bars.length / MAX_TICKS));
    return bars.filter((_, index) => index % step === 0).map((bar) => bar.time);
  }, [bars]);

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
    <Box sx={{ height, cursor: onZoom ? 'zoom-in' : 'default' }}>
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
          format: (value) => formatBucket(new Date(String(value)), data.interval),
        }}
        onClick={(bar) => {
          const from = new Date(String(bar.indexValue));
          onZoom?.(from, new Date(from.getTime() + width));
        }}
        tooltip={({ indexValue, value }) => (
          <div
            style={{
              padding: '8px 10px',
              background: getTooltipBackground(),
              color: getTooltipTextColor(),
              borderRadius: 4,
              fontSize: 12,
              fontFamily: fontFamilyBody,
              boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
              whiteSpace: 'nowrap',
            }}
          >
            <div style={{ fontWeight: 600 }}>
              {formatCount(Number(value))} event{Number(value) === 1 ? '' : 's'}
            </div>
            <div>{formatTimestamp(new Date(String(indexValue)))}</div>
            <div style={{ color: colorInk40 }}>per {data.interval}</div>
          </div>
        )}
        theme={{
          axis: { ticks: { text: { fill: colorInk40, fontFamily: fontFamilyBody, fontSize: 11 } } },
          grid: { line: { stroke: getAxisColor(), strokeDasharray: '4 4' } },
        }}
        animate={false}
        role="img"
        ariaLabel="Events over time"
      />
    </Box>
  );
}
