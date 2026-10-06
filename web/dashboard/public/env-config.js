// Runtime settings, read by src/config/env.ts. The Docker image overwrites
// this file at container start with the container's VITE_* environment
// variables (see env-config.sh). Left empty here, so `npm run dev` uses .env.
window.__ENV__ = {};
