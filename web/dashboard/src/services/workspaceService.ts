import { apiClient } from './apiClient';

/**
 * Where a workspace is in provisioning its infrastructure. Only an `ACTIVE`
 * workspace can be queried; an operator provisions new workspaces.
 */
export type WorkspaceStatus = 'PENDING' | 'CREATING' | 'ERRORED' | 'ACTIVE' | 'DELETING';

export interface Workspace {
  id: string;
  organization_id: string | null;
  name: string;
  status: WorkspaceStatus;
  workspace_token?: string;
  timezone?: string;
  created_at: string;
}

interface CreateWorkspaceRequest {
  name: string;
}

interface UpdateWorkspaceRequest {
  name?: string;
  timezone?: string;
}

interface AddWorkspaceMemberRequest {
  email: string;
  role: 'workspace_admin' | 'workspace_user';
}

interface WorkspaceMember {
  user_id: string;
  email: string;
  username: string;
  role: 'workspace_admin' | 'workspace_user';
}

interface WorkspaceToken {
  id: string;
  token: string;
  name: string;
  last_used_at: string | null;
  created_at: string;
}

interface AddWorkspaceMemberResponse {
  user_id: string;
  workspace_id: string;
  role: 'workspace_admin' | 'workspace_user';
}

function orgHeader(organizationId: string): Record<string, string> {
  return { 'X-Organization-Id': organizationId };
}

export const workspaceService = {
  async create(organizationId: string, data: CreateWorkspaceRequest): Promise<Workspace> {
    return apiClient.post('/workspaces', data, {
      headers: orgHeader(organizationId),
    });
  },

  async list(organizationId: string): Promise<Workspace[]> {
    return apiClient.get('/workspaces', {}, {
      headers: orgHeader(organizationId),
    });
  },

  async get(organizationId: string, workspaceId: string): Promise<Workspace> {
    return apiClient.get('/workspace', {}, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async delete(organizationId: string, workspaceId: string): Promise<void> {
    await apiClient.delete('/workspace', {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async update(organizationId: string, workspaceId: string, data: UpdateWorkspaceRequest): Promise<Workspace> {
    return apiClient.patch('/workspace', data, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async addMember(organizationId: string, workspaceId: string, data: AddWorkspaceMemberRequest): Promise<AddWorkspaceMemberResponse> {
    return apiClient.post('/workspace/members', data, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async listMembers(organizationId: string, workspaceId: string): Promise<WorkspaceMember[]> {
    return apiClient.get('/workspace/members', {}, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async removeMember(organizationId: string, workspaceId: string, userId: string): Promise<void> {
    await apiClient.delete(`/workspace/members/${userId}`, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async updateMemberRole(organizationId: string, workspaceId: string, userId: string, role: 'workspace_admin' | 'workspace_user'): Promise<void> {
    await apiClient.post(`/workspace/members/${userId}/role`, { role }, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async getWorkspaceTokens(organizationId: string, workspaceId: string): Promise<Record<string, WorkspaceToken>> {
    return apiClient.get('/workspace/tokens', {}, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async getEventsTodayCount(organizationId: string, workspaceId: string): Promise<number> {
    const result = await apiClient.get<{ count: number }>('/events/today/count', {}, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
    return result.count;
  },
};
