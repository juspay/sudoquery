import { authService } from './authService';
import { API_CONFIG } from '../config/api';
import { ORG_STORAGE_KEY, PROJECT_STORAGE_KEY } from '../config/storage';

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public data?: unknown
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

interface RequestOptions extends Omit<RequestInit, 'headers'> {
  headers?: Record<string, string>;
  params?: Record<string, string>;
  skipAuth?: boolean;
}

/**
 * Centralized API client that automatically adds Bearer token to all requests.
 * Handles 401 errors by clearing auth state and redirecting to home.
 */
class ApiClient {
  private baseUrl: string;

  constructor() {
    this.baseUrl = API_CONFIG.API_BASE;
  }

  /**
   * Get organization ID from localStorage
   */
  private getOrganizationId(): string | null {
    try {
      return localStorage.getItem(ORG_STORAGE_KEY);
    } catch {
      return null;
    }
  }

  /**
   * Get project ID from localStorage
   */
  private getProjectId(): string | null {
    try {
      return localStorage.getItem(PROJECT_STORAGE_KEY);
    } catch {
      return null;
    }
  }

  /**
   * Get tenant/organization/project headers
   */
  private getTenantHeaders(): Record<string, string> {
    const headers: Record<string, string> = {};

    const orgId = this.getOrganizationId();
    if (orgId) {
      headers['X-Organization-Id'] = orgId;
    }

    const projectId = this.getProjectId();
    if (projectId) {
      headers['X-Project-Id'] = projectId;
    }

    return headers;
  }

  /**
   * Get the base URL for API requests
   */
  getBaseUrl(): string {
    return this.baseUrl;
  }

  /**
   * Build full URL with query params
   */
  private buildUrl(endpoint: string, params?: Record<string, string>): string {
    // If endpoint is already a full URL, use it as-is
    const url = endpoint.startsWith('http')
      ? endpoint
      : `${this.baseUrl}${endpoint}`;

    if (!params || Object.keys(params).length === 0) {
      return url;
    }

    const searchParams = new URLSearchParams(params);
    const separator = url.includes('?') ? '&' : '?';
    return `${url}${separator}${searchParams.toString()}`;
  }

  /**
   * Get auth headers
   */
  private getAuthHeaders(): Record<string, string> {
    const auth = authService.getStoredAuth();
    if (auth?.token) {
      return { Authorization: `Bearer ${auth.token}` };
    }
    return {};
  }

  /**
   * Handle 401 Unauthorized - clear auth and redirect to home
   */
  private handleUnauthorized(): void {
	  console.log("~~handleUnauthorized");
    authService.logout();
    // Only redirect if not already on home/callback
    if (window.location.pathname !== '/' && window.location.pathname !== '/callback') {
      window.location.href = '/';
    }
  }

  /**
   * Make an API request with automatic auth header injection
   */
  async request<T = unknown>(
    endpoint: string,
    options: RequestOptions = {}
  ): Promise<T> {
    const { params, skipAuth = false, headers = {}, ...fetchOptions } = options;

	console.log("~~ ", endpoint, options);
    const url = this.buildUrl(endpoint, params);

    // Merge headers: default < tenant headers < auth headers < custom headers
    const finalHeaders: Record<string, string> = {
      'Content-Type': 'application/json',
      ...this.getTenantHeaders(),
      ...(skipAuth ? {} : this.getAuthHeaders()),
      ...headers,
    };

    // Remove Content-Type for FormData
    if (fetchOptions.body instanceof FormData) {
      delete finalHeaders['Content-Type'];
    }

    const response = await fetch(url, {
      ...fetchOptions,
      headers: finalHeaders,
    });

    // Handle 401 Unauthorized
    if (response.status === 401 && !skipAuth) {
	console.log("~~ unauth", endpoint, options);
      this.handleUnauthorized();
      throw new ApiError(401, 'Unauthorized');
    }

    // Handle other errors
    if (!response.ok) {
      const errorText = await response.text();
      let errorData: unknown;
      try {
        errorData = JSON.parse(errorText);
      } catch {
        errorData = { message: errorText };
      }
      throw new ApiError(
        response.status,
        errorText || `Request failed with status ${response.status}`,
        errorData
      );
    }

    // Handle empty response
    const contentType = response.headers.get('content-type');
    if (contentType?.includes('application/json')) {
      return response.json();
    }

    // Return text for non-JSON responses
    const text = await response.text();
    return text as unknown as T;
  }

  /**
   * GET request
   */
  async get<T = unknown>(
    endpoint: string,
    params?: Record<string, string>,
    options?: RequestOptions
  ): Promise<T> {
    return this.request<T>(endpoint, {
      ...options,
      method: 'GET',
      params,
    });
  }

  /**
   * POST request
   */
  async post<T = unknown>(
    endpoint: string,
    data?: unknown,
    options?: RequestOptions
  ): Promise<T> {
    return this.request<T>(endpoint, {
      ...options,
      method: 'POST',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  /**
   * PUT request
   */
  async put<T = unknown>(
    endpoint: string,
    data?: unknown,
    options?: RequestOptions
  ): Promise<T> {
    return this.request<T>(endpoint, {
      ...options,
      method: 'PUT',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  /**
   * PATCH request
   */
  async patch<T = unknown>(
    endpoint: string,
    data?: unknown,
    options?: RequestOptions
  ): Promise<T> {
    return this.request<T>(endpoint, {
      ...options,
      method: 'PATCH',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  /**
   * DELETE request
   */
  async delete<T = unknown>(
    endpoint: string,
    options?: RequestOptions
  ): Promise<T> {
    return this.request<T>(endpoint, {
      ...options,
      method: 'DELETE',
    });
  }

  /**
   * Stream request for SSE endpoints
   * Returns raw Response for streaming consumption
   */
  async stream(
    endpoint: string,
    data?: unknown,
    options?: RequestOptions
  ): Promise<Response> {
    const { params, skipAuth = false, headers = {}, ...fetchOptions } = options || {};

    const url = this.buildUrl(endpoint, params);

    // Merge headers: default < tenant headers < auth headers < custom headers
    const finalHeaders: Record<string, string> = {
      'Content-Type': 'application/json',
      ...this.getTenantHeaders(),
      ...(skipAuth ? {} : this.getAuthHeaders()),
      ...headers,
    };

    const response = await fetch(url, {
      ...fetchOptions,
      method: 'POST',
      headers: finalHeaders,
      body: data ? JSON.stringify(data) : undefined,
    });

    // Handle 401 Unauthorized
    if (response.status === 401 && !skipAuth) {
      this.handleUnauthorized();
      throw new ApiError(401, 'Unauthorized');
    }

    // Handle other errors
    if (!response.ok) {
      const errorText = await response.text();
      let errorData: unknown;
      try {
        errorData = JSON.parse(errorText);
      } catch {
        errorData = { message: errorText };
      }
      throw new ApiError(
        response.status,
        errorText || `Stream request failed with status ${response.status}`,
        errorData
      );
    }

    return response;
  }
}

// Export singleton instance
export const apiClient = new ApiClient();
