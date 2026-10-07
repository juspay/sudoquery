/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL: string;
  readonly VITE_KEYCLOAK_URL: string;
  readonly VITE_KEYCLOAK_REALM: string;
  readonly VITE_KEYCLOAK_CLIENT_ID: string;
  readonly VITE_KEYCLOAK_REDIRECT_URI: string;
  readonly VITE_KEYCLOAK_IDP_ALIAS: string;
  readonly VITE_ANALYTICS_TOKEN: string;
  readonly VITE_ANALYTICS_FLUSH_INTERVAL: string;
  readonly VITE_MAINTENANCE_MODE?: string;
  readonly VITE_DEMO_ENABLED?: string;
  readonly VITE_DEMO_COLLECTOR_URL?: string;
  readonly VITE_DEMO_TENANT_ID?: string;
  readonly VITE_DEMO_WORKSPACE_ID?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

interface Window {
  /** Runtime VITE_* settings from /env-config.js; see src/config/env.ts. */
  readonly __ENV__?: Readonly<Record<string, string>>;
}
