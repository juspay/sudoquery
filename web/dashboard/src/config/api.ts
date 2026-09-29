const getEnvVar = (key: string, defaultValue: string): string => {
  const value = import.meta.env[key];
  return value ?? defaultValue;
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
