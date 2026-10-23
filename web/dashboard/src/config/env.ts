/**
 * Look up a VITE_* setting. Values from /env-config.js, which the Docker image
 * writes from the container's environment at startup (see env-config.sh), win
 * over the ones Vite baked in at build time, so one image can be configured
 * per deployment. Empty strings count as unset.
 *
 * `{hostname}` in a value is replaced with the hostname the page was loaded
 * from, so e.g. `http://{hostname}:3000/api` reaches the API on the same
 * machine whether the dashboard is opened via localhost or a tailnet address.
 */
export const getEnv = (key: string): string | undefined => {
  const value = globalThis.window?.__ENV__?.[key] || import.meta.env[key] || undefined;
  const hostname = globalThis.window?.location?.hostname;
  return value && hostname ? value.replaceAll('{hostname}', hostname) : value;
};
