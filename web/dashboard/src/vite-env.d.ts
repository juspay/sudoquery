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
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
