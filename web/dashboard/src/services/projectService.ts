import { apiClient } from './apiClient';

export interface Project {
  id: string;
  organization_id: string | null;
  name: string;
  project_token?: string;
  timezone?: string;
  created_at: string;
}

interface CreateProjectRequest {
  name: string;
}

interface UpdateProjectRequest {
  name?: string;
  timezone?: string;
}

interface AddProjectMemberRequest {
  email: string;
  role: 'project_admin' | 'project_user';
}

interface ProjectMember {
  user_id: string;
  email: string;
  username: string;
  role: 'project_admin' | 'project_user';
}

interface ProjectToken {
  id: string;
  token: string;
  name: string;
  last_used_at: string | null;
  created_at: string;
}

interface AddProjectMemberResponse {
  user_id: string;
  project_id: string;
  role: 'project_admin' | 'project_user';
}

function orgHeader(organizationId: string): Record<string, string> {
  return { 'X-Organization-Id': organizationId };
}

export const projectService = {
  async create(organizationId: string, data: CreateProjectRequest): Promise<Project> {
    return apiClient.post('/projects', data, {
      headers: orgHeader(organizationId),
    });
  },

  async list(organizationId: string): Promise<Project[]> {
    return apiClient.get('/projects', {}, {
      headers: orgHeader(organizationId),
    });
  },

  async delete(organizationId: string, projectId: string): Promise<void> {
    await apiClient.delete('/project', {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async update(organizationId: string, projectId: string, data: UpdateProjectRequest): Promise<Project> {
    return apiClient.patch('/project', data, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async addMember(organizationId: string, projectId: string, data: AddProjectMemberRequest): Promise<AddProjectMemberResponse> {
    return apiClient.post('/project/members', data, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async listMembers(organizationId: string, projectId: string): Promise<ProjectMember[]> {
    return apiClient.get('/project/members', {}, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async removeMember(organizationId: string, projectId: string, userId: string): Promise<void> {
    await apiClient.delete(`/project/members/${userId}`, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async updateMemberRole(organizationId: string, projectId: string, userId: string, role: 'project_admin' | 'project_user'): Promise<void> {
    await apiClient.post(`/project/members/${userId}/role`, { role }, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async getProjectTokens(organizationId: string, projectId: string): Promise<Record<string, ProjectToken>> {
    return apiClient.get('/project/tokens', {}, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
  },

  async getEventsTodayCount(organizationId: string, projectId: string): Promise<number> {
    const result = await apiClient.get<{ count: number }>('/events/today/count', {}, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
    });
    return result.count;
  },
};
