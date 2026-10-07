import { getEnv } from './env';

const getEnvVar = (key: string, defaultValue: string): string => {
  return getEnv(key) ?? defaultValue;
};

export const API_CONFIG = {
  /**
   * Backend API base URL
   * Development: Set to backend server URL (e.g., http://localhost:3000)
   * Production: Can be empty for same-origin or set to actual backend URL
   */
  get API_BASE() {
    return getEnvVar('VITE_API_BASE_URL', '');
  },
};

export const KEYCLOAK_CONFIG = {
  get URL() {
    return getEnvVar('VITE_KEYCLOAK_URL', 'http://localhost:8080');
  },
  get REALM() {
    return getEnvVar('VITE_KEYCLOAK_REALM', 'hyper-analytics');
  },
  get CLIENT_ID() {
    return getEnvVar('VITE_KEYCLOAK_CLIENT_ID', 'hyper-analytics-ui');
  },
  get REDIRECT_URI() {
    return getEnvVar('VITE_KEYCLOAK_REDIRECT_URI', `${window.location.origin}/callback`);
  },
  get POST_LOGOUT_URI() {
    return getEnvVar('VITE_KEYCLOAK_POST_LOGOUT_URI', window.location.origin);
  },
  get IDP_ALIAS() {
    return getEnvVar('VITE_KEYCLOAK_IDP_ALIAS', 'google');
  },
};

/** The demo store at /demo, which sends its events to an events collector. */
export const DEMO_CONFIG = {
  /** The /demo route exists only when this is `true`. */
  get ENABLED() {
    return getEnv('VITE_DEMO_ENABLED') === 'true';
  },
  /** The collector's batch endpoint. */
  get COLLECTOR_URL() {
    return getEnvVar('VITE_DEMO_COLLECTOR_URL', 'http://localhost:3000/v1/events/batch');
  },
  /** Default organization ID (`org_id`); the page can change it. */
  get TENANT_ID() {
    return getEnv('VITE_DEMO_TENANT_ID');
  },
  /** Default project UUID (`proj_id`); the page can change it. */
  get WORKSPACE_ID() {
    return getEnv('VITE_DEMO_WORKSPACE_ID');
  },
};
