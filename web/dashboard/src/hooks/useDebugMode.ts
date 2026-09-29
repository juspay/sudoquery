import { useState, useEffect } from 'react';

const STORAGE_KEY = 'hyper_analytics_debug_mode';

export function useDebugMode() {
  const [enabled, setEnabled] = useState(() => {
    try {
      return localStorage.getItem(STORAGE_KEY) === 'true';
    } catch {
      return false;
    }
  });

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, String(enabled));
    } catch {}
  }, [enabled]);

  return { debugMode: enabled, setDebugMode: setEnabled };
}
