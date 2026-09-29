import { API_CONFIG, KEYCLOAK_CONFIG } from '../config/api';
import { apiClient } from './apiClient';

const API_BASE = API_CONFIG.API_BASE;

export interface User {
  sub: string;
  email?: string;
  preferred_username?: string;
  realm_id?: string;
  roles: string[];
}

export interface AuthState {
  isAuthenticated: boolean;
  isAdmin: boolean;
  token: string | null;
  user: User | null;
}

interface LoginResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
}

interface TokenResponse {
  access_token: string;
  refresh_token: string;
  id_token: string;
  expires_in: number;
  token_type: string;
}

interface Project {
  id: string;
  realm_id: string;
  name: string;
  project_token?: string;
  created_at: string;
}

interface Tenant {
  id: string;
  realm_id: string;
  name: string;
  created_at: string;
}

interface KeycloakUser {
  id: string;
  username: string;
  email?: string;
  enabled: boolean;
}

function parseJwt(token: string): User {
  const base64Url = token.split('.')[1];
  const base64 = base64Url.replace(/-/g, '+').replace(/_/g, '/');
  const jsonPayload = decodeURIComponent(
    atob(base64)
      .split('')
      .map((c) => '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2))
      .join('')
  );
  const payload = JSON.parse(jsonPayload);

  const realmId = payload.iss?.split('/realms/')[1] || '';

  return {
    sub: payload.sub,
    email: payload.email,
    preferred_username: payload.preferred_username,
    realm_id: realmId,
    roles: payload.realm_access?.roles || [],
  };
}

export const authService = {
  async adminLogin(username: string, password: string): Promise<AuthState> {
    const data: LoginResponse = await apiClient.post('/auth/login', { username, password }, { skipAuth: true });
    const user = parseJwt(data.access_token);

    const authState: AuthState = {
      isAuthenticated: true,
      isAdmin: user.roles.includes('super_admin'),
      token: data.access_token,
      user,
    };

    localStorage.setItem('auth_state', JSON.stringify(authState));
    return authState;
  },

  async userLogin(username: string, password: string, realm: string): Promise<AuthState> {
    const data: LoginResponse = await apiClient.post('/auth/login', { username, password, realm }, { skipAuth: true });
    const user = parseJwt(data.access_token);

    const authState: AuthState = {
      isAuthenticated: true,
      isAdmin: false,
      token: data.access_token,
      user,
    };

    localStorage.setItem('auth_state', JSON.stringify(authState));
    return authState;
  },

  logout(): void {
    console.log("~~removing auth");
    const idToken = localStorage.getItem('id_token');
    localStorage.removeItem('auth_state');
    localStorage.removeItem('refresh_token');
    localStorage.removeItem('id_token');

    if (idToken) {
      const logoutUrl = `${KEYCLOAK_CONFIG.URL}/realms/${KEYCLOAK_CONFIG.REALM}/protocol/openid-connect/logout?id_token_hint=${idToken}&post_logout_redirect_uri=${encodeURIComponent(KEYCLOAK_CONFIG.POST_LOGOUT_URI)}`;
      window.location.href = logoutUrl;
    } else {
      window.location.href = '/';
    }
  },

  getStoredAuth(): AuthState | null {
    try {
      const stored = localStorage.getItem('auth_state');
      if (stored) {
        return JSON.parse(stored);
      }
    } catch {
      return null;
    }
    return null;
  },

  getAuthHeaders(): Record<string, string> {
    const auth = this.getStoredAuth();
    if (auth?.token) {
      return { Authorization: `Bearer ${auth.token}` };
    }
    return {};
  },

  /**
   * Initiate Google OAuth login via Keycloak
   * Redirects directly to Google (skips Keycloak login UI)
   * @param idpAlias - The Keycloak IdP alias (default: 'google'). Check Keycloak Admin Console for actual alias.
   */
  initiateGoogleLogin(idpAlias: string = 'google'): void {
    const params = new URLSearchParams({
      client_id: KEYCLOAK_CONFIG.CLIENT_ID,
      redirect_uri: KEYCLOAK_CONFIG.REDIRECT_URI,
      response_type: 'code',
      scope: 'openid email profile',
      kc_idp_hint: idpAlias,
      prompt: 'login',
    });

    const authUrl = `${KEYCLOAK_CONFIG.URL}/realms/${KEYCLOAK_CONFIG.REALM}/protocol/openid-connect/auth?${params}`;
    window.location.href = authUrl;
  },

  /**
   * Initiate login with specific identity provider directly
   * Redirects directly to the IdP (bypasses Keycloak login page entirely)
   * @param idpAlias - The Keycloak IdP alias
   */
  initiateLoginWithIdp(idpAlias: string): void {
    const params = new URLSearchParams({
      client_id: KEYCLOAK_CONFIG.CLIENT_ID,
      redirect_uri: KEYCLOAK_CONFIG.REDIRECT_URI,
      response_type: 'code',
      scope: 'openid email profile',
    });

    // Direct IdP redirect - bypasses Keycloak login completely
    const authUrl = `${KEYCLOAK_CONFIG.URL}/realms/${KEYCLOAK_CONFIG.REALM}/protocol/openid-connect/auth?${params}&kc_idp=${idpAlias}`;
    window.location.href = authUrl;
  },

  /**
   * Handle OAuth callback - exchange authorization code for tokens
   */
  async handleOAuthCallback(code: string): Promise<AuthState> {
    const tokenUrl = `${KEYCLOAK_CONFIG.URL}/realms/${KEYCLOAK_CONFIG.REALM}/protocol/openid-connect/token`;

    const response = await fetch(tokenUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        grant_type: 'authorization_code',
        code,
        client_id: KEYCLOAK_CONFIG.CLIENT_ID,
        redirect_uri: KEYCLOAK_CONFIG.REDIRECT_URI,
      }),
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Token exchange failed: ${error}`);
    }

    const data: TokenResponse = await response.json();
    const user = parseJwt(data.access_token);

    const authState: AuthState = {
      isAuthenticated: true,
      isAdmin: user.roles.includes('super_admin'),
      token: data.access_token,
      user,
    };

    localStorage.setItem('refresh_token', data.refresh_token);
    localStorage.setItem('id_token', data.id_token);
    localStorage.setItem('auth_state', JSON.stringify(authState));
    return authState;
  },

  /**
   * Refresh access token using refresh token
   */
  async refreshAccessToken(): Promise<AuthState | null> {
    const refreshToken = localStorage.getItem('refresh_token');
    if (!refreshToken) return null;

    const tokenUrl = `${KEYCLOAK_CONFIG.URL}/realms/${KEYCLOAK_CONFIG.REALM}/protocol/openid-connect/token`;

    try {
      const response = await fetch(tokenUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          grant_type: 'refresh_token',
          refresh_token: refreshToken,
          client_id: KEYCLOAK_CONFIG.CLIENT_ID,
        }),
      });

      if (!response.ok) {
        this.logout();
        return null;
      }

      const data: TokenResponse = await response.json();
      const user = parseJwt(data.access_token);

      const authState: AuthState = {
        isAuthenticated: true,
        isAdmin: user.roles.includes('super_admin'),
        token: data.access_token,
        user,
      };

      localStorage.setItem('refresh_token', data.refresh_token);
      localStorage.setItem('auth_state', JSON.stringify(authState));
      return authState;
    } catch {
      this.logout();
      return null;
    }
  },
};

const adminService = {
  async createTenant(name: string): Promise<Tenant> {
    return apiClient.post('/tenants', { name });
  },

  async deleteTenant(tenantId: string): Promise<void> {
    await apiClient.delete(`/tenants/${tenantId}`);
  },

  async createUser(realmId: string, username: string, email: string, password: string): Promise<KeycloakUser> {
    return apiClient.post('/users', { realm_id: realmId, username, email, password });
  },

  async deleteUser(userId: string): Promise<void> {
    await apiClient.delete(`/users/${userId}`);
  },

  async createProject(name: string): Promise<Project> {
    return apiClient.post('/projects', { name });
  },

  async listProjects(): Promise<Project[]> {
    return apiClient.get('/projects');
  },

  async deleteProject(projectId: string): Promise<void> {
    await apiClient.delete(`/projects/${projectId}`);
  },

  async getCurrentUser(): Promise<KeycloakUser> {
    return apiClient.get('/me');
  },
};
