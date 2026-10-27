import { apiClient } from './apiClient';
import type { JsonValue } from '../types/funnel';

export interface Dashboard {
  id: string;
  workspace_id: string;
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

function headers(organizationId: string, workspaceId: string): Record<string, string> {
  return {
    'X-Organization-Id': organizationId,
    'X-Workspace-Id': workspaceId,
  };
}

export const dashboardService = {
  async list(organizationId: string, workspaceId: string): Promise<Dashboard[]> {
    const response = await apiClient.get<ListDashboardsResponse>('/workspace/live-dashboards', {}, {
      headers: headers(organizationId, workspaceId),
    });
    return response.dashboards;
  },

  async get(organizationId: string, workspaceId: string, dashboardId: string): Promise<Dashboard> {
    return apiClient.get<Dashboard>('/live-dashboard', { dashboard_id: dashboardId }, {
      headers: headers(organizationId, workspaceId),
    });
  },

  async create(organizationId: string, workspaceId: string, data: CreateDashboardRequest): Promise<Dashboard> {
    return apiClient.post<Dashboard>('/workspace/live-dashboards', data, {
      headers: headers(organizationId, workspaceId),
    });
  },

  async update(organizationId: string, workspaceId: string, data: UpdateDashboardRequest): Promise<Dashboard> {
    return apiClient.patch<Dashboard>('/live-dashboard', data, {
      headers: headers(organizationId, workspaceId),
    });
  },

  async delete(organizationId: string, workspaceId: string, dashboardId: string): Promise<void> {
    await apiClient.delete('/live-dashboard', {
      headers: headers(organizationId, workspaceId),
      params: { dashboard_id: dashboardId },
    });
  },

  async saveDashboardFromChat(
    chatId: string,
    messageId: string,
    toolCallId: string
  ): Promise<{ dashboard_id: string }> {
    return apiClient.post<{ dashboard_id: string }>(`/workspace/chat/${chatId}/save-dashboard`, {
      message_id: messageId,
      tool_call_id: toolCallId,
    });
  },
};
