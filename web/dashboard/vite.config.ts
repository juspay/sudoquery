import { fileURLToPath } from 'node:url'
import { defineConfig, searchForWorkspaceRoot } from 'vite'
import react from '@vitejs/plugin-react'

// The TypeScript SDK in this repo, which the demo store at /demo runs on (see
// src/demo/analytics.ts). The rest of the dashboard uses the published
// sudo-query package.
const sdkSource = fileURLToPath(new URL('../../clients/typescript/src', import.meta.url))

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@clients/typescript': `${sdkSource}/index.ts`,
    },
  },
  server: {
    fs: {
      // The SDK source sits outside this package, which the dev server
      // otherwise refuses to serve.
      allow: [searchForWorkspaceRoot(process.cwd()), sdkSource],
    },
  },
  build: {
    rollupOptions: {
      maxParallelFileOps: 20,
    },
  },
})
