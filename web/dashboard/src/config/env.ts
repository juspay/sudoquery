/**
 * Look up a VITE_* setting. Values from /env-config.js, which the Docker image
 * writes from the container's environment at startup (see env-config.sh), win
 * over the ones Vite baked in at build time, so one image can be configured
 * per deployment. Empty strings count as unset.
 */
export const getEnv = (key: string): string | undefined =>
  globalThis.window?.__ENV__?.[key] || import.meta.env[key] || undefined;
