// src/index.ts
export const add = (a: number, b: number): number => {
  return a + b;
};

export const logMessage = (msg: string) => {
  console.log(`[MyLib]: ${msg}`);
};

// Export types
export type { JSONSerializable } from './types';

// Export TypeValidator functions
export { containsNonPrimitives } from './TypeValidator';

// Export main classes
export { HyperAnalytics } from './HyperAnalytics';
export { Configuration } from './Configuration';
export { SuperProperties } from './SuperProperties';
export { flush } from './Flush';
export { Pusher } from './Pusher';
export { Batcher } from './Batcher';
