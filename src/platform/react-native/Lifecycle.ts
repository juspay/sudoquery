/**
 * React Native lifecycle adapter using AppState.
 */
import type { LifecycleAdapter } from '../types';

type AppStateStatus = 'active' | 'background' | 'inactive' | 'unknown' | 'extension';

interface AppStateStatic {
  currentState: AppStateStatus;
  addEventListener: (
    type: 'change',
    handler: (state: AppStateStatus) => void
  ) => { remove: () => void };
}

let AppState: AppStateStatic | null = null;
let AppStatePromise: Promise<AppStateStatic> | null = null;

function getAppState(): Promise<AppStateStatic> {
  if (AppState) return Promise.resolve(AppState);
  if (AppStatePromise) return AppStatePromise;

  AppStatePromise = (async () => {
    try {
      const rn = await import('react-native');
      AppState = rn.AppState;
      if (!AppState) {
        throw new Error('AppState not available in react-native');
      }
      return AppState;
    } catch {
      throw new Error(
        'react-native is required. Install it with: npm install react-native'
      );
    }
  })();

  return AppStatePromise;
}

export function createLifecycle(): LifecycleAdapter {
  return {
    onBackground(callback: () => void): () => void {
      let subscription: { remove: () => void } | null = null;

      getAppState().then(appState => {
        subscription = appState.addEventListener('change', (state) => {
          if (state === 'background' || state === 'inactive') {
            callback();
          }
        });
      }).catch(() => {});

      return () => {
        subscription?.remove();
      };
    },

    onForeground(callback: () => void): () => void {
      let subscription: { remove: () => void } | null = null;

      getAppState().then(appState => {
        subscription = appState.addEventListener('change', (state) => {
          if (state === 'active') {
            callback();
          }
        });
      }).catch(() => {});

      return () => {
        subscription?.remove();
      };
    },

    onTerminate(callback: () => void): () => void {
      // React Native doesn't have a native terminate event
      // Use background event as approximation and persist events
      let subscription: { remove: () => void } | null = null;

      getAppState().then(appState => {
        subscription = appState.addEventListener('change', (state) => {
          if (state === 'background') {
            // This is our best chance to flush/persist before termination
            callback();
          }
        });
      }).catch(() => {});

      return () => {
        subscription?.remove();
      };
    },
  };
}