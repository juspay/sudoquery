/**
 * React Native lifecycle adapter.
 *
 * IMPORTANT: This adapter does NOT import react-native to avoid native module errors in Expo Go.
 * Lifecycle events (background/foreground) are not supported in Expo Go.
 *
 * For bare React Native apps, you can extend this to use AppState from react-native.
 */
import type { LifecycleAdapter } from '../types';

export function createLifecycle(): LifecycleAdapter {
  // Note: We intentionally do NOT import react-native here
  // because it causes native module errors in Expo Go.
  // Lifecycle events will be no-ops in Expo Go.

  // If you need lifecycle events in bare React Native, you can
  // manually set up AppState listeners in your app code:
  //
  // import { AppState } from 'react-native';
  // AppState.addEventListener('change', (state) => {
  //   if (state === 'background') {
  //     HyperAnalytics.flush();
  //   }
  // });

  return {
    onBackground(_callback: () => void): () => void {
      // No-op in Expo Go - react-native AppState not available
      return () => {};
    },

    onForeground(_callback: () => void): () => void {
      // No-op in Expo Go - react-native AppState not available
      return () => {};
    },

    onTerminate(_callback: () => void): () => void {
      // No-op in Expo Go - react-native AppState not available
      return () => {};
    },
  };
}