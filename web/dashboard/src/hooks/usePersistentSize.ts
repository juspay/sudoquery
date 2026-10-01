import { useCallback, useState } from 'react';

/**
 * A pixel size the user can change, remembered in this browser. Falls back to
 * `defaultSize` when nothing valid is stored or storage is unavailable.
 */
export function usePersistentSize(key: string, defaultSize: number, min: number, max: number) {
  const clamp = useCallback((size: number) => Math.min(max, Math.max(min, size)), [min, max]);

  const [size, setSize] = useState(() => {
    try {
      const stored = Number(localStorage.getItem(key));
      return stored ? clamp(stored) : defaultSize;
    } catch {
      return defaultSize;
    }
  });

  const update = useCallback(
    (next: number) => {
      const clamped = clamp(Math.round(next));
      setSize(clamped);
      try {
        localStorage.setItem(key, String(clamped));
      } catch {
        // Storage unavailable (private window, blocked): the size lasts until reload.
      }
    },
    [key, clamp],
  );

  const reset = useCallback(() => {
    setSize(defaultSize);
    try {
      localStorage.removeItem(key);
    } catch {
      // Nothing stored to clear.
    }
  }, [key, defaultSize]);

  return { size, setSize: update, reset, min, max };
}
