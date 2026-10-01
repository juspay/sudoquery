import { useEffect, useEffectEvent } from 'react';
import { intervalMs } from './timeRange';

/** The intervals offered, as they appear in the page URL. */
export const REFRESH_INTERVALS: Array<{ value: string; label: string }> = [
  { value: '5s', label: '5 seconds' },
  { value: '10s', label: '10 seconds' },
  { value: '30s', label: '30 seconds' },
  { value: '1m', label: '1 minute' },
  { value: '5m', label: '5 minutes' },
  { value: '15m', label: '15 minutes' },
  { value: '30m', label: '30 minutes' },
  { value: '1h', label: '1 hour' },
];

/** `null` for anything that is not one of the offered intervals. */
export function parseRefreshInterval(value: string | null): string | null {
  return REFRESH_INTERVALS.some((option) => option.value === value) ? value : null;
}

/**
 * Calls `refresh` every `interval` while the page is visible. A hidden tab
 * makes no requests, and catches up as soon as it is shown again if a
 * refresh came due meanwhile.
 */
export function useAutoRefresh(interval: string | null, refresh: () => void) {
  const tick = useEffectEvent(refresh);

  useEffect(() => {
    const ms = interval ? intervalMs(interval) : null;
    if (!ms) return;

    let due = false;
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') {
        tick();
      } else {
        due = true;
      }
    }, ms);
    const onVisible = () => {
      if (due && document.visibilityState === 'visible') {
        due = false;
        tick();
      }
    };
    document.addEventListener('visibilitychange', onVisible);

    return () => {
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [interval]);
}
