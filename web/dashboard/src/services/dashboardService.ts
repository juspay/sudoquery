import { apiClient } from './apiClient';
import type { JsonValue } from '../types/funnel';

export interface Dashboard {
  id: string;
  project_id: string;
  query: string;
  description: string;
  chart_config: string | null;
  last_ran_at: string | null;
  response: JsonValue | null;
  created_at: string;
  updated_at: string;
}

interface CreateDashboardRequest {
  query: string;
  description: string;
  chart_config?: string;
}

interface UpdateDashboardRequest {
  dashboard_id: string;
  query: string;
  description: string;
  chart_config?: string;
}

interface ListDashboardsResponse {
  dashboards: Dashboard[];
}

function headers(organizationId: string, projectId: string): Record<string, string> {
  return {
    'X-Organization-Id': organizationId,
    'X-Project-Id': projectId,
  };
}

export const dashboardService = {
  async list(organizationId: string, projectId: string): Promise<Dashboard[]> {
    const response = await apiClient.get<ListDashboardsResponse>('/project/live-dashboards', {}, {
      headers: headers(organizationId, projectId),
    });
    return response.dashboards;
  },

  async get(organizationId: string, projectId: string, dashboardId: string): Promise<Dashboard> {
    return apiClient.get<Dashboard>('/live-dashboard', { dashboard_id: dashboardId }, {
      headers: headers(organizationId, projectId),
    });
  },

  async create(organizationId: string, projectId: string, data: CreateDashboardRequest): Promise<Dashboard> {
    return apiClient.post<Dashboard>('/project/live-dashboards', data, {
      headers: headers(organizationId, projectId),
    });
  },

  async update(organizationId: string, projectId: string, data: UpdateDashboardRequest): Promise<Dashboard> {
    return apiClient.patch<Dashboard>('/live-dashboard', data, {
      headers: headers(organizationId, projectId),
    });
  },

  async delete(organizationId: string, projectId: string, dashboardId: string): Promise<void> {
    await apiClient.delete('/live-dashboard', {
      headers: headers(organizationId, projectId),
      params: { dashboard_id: dashboardId },
    });
  },

  async saveDashboardFromChat(
    chatId: string,
    messageId: string,
    toolCallId: string
  ): Promise<{ dashboard_id: string }> {
    return apiClient.post<{ dashboard_id: string }>(`/project/chat/${chatId}/save-dashboard`, {
      message_id: messageId,
      tool_call_id: toolCallId,
    });
  },
};
