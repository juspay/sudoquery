import { apiClient } from './apiClient';

type AccessLevel =
  | 'org_admin'
  | 'org_user'
  | 'project_project_admin'
  | 'project_project_user';

export interface Organization {
  id: string;
  slug: string;
  name: string;
  created_at: string;
  /** How the current user has access to this organization */
  access_level?: AccessLevel;
}

interface OrgMember {
  user_id: string;
  email: string;
  username: string;
  role: 'org_admin' | 'org_user';
}

interface CreateOrganizationRequest {
  slug: string;
  name: string;
}

interface UpdateOrganizationRequest {
  name?: string;
  slug?: string;
}

interface AddOrgMemberRequest {
  user_id: string;
  role: 'org_admin' | 'org_user';
}

export const organizationService = {
  /**
   * Create a new organization
   * User becomes org_admin automatically
   */
  async create(data: CreateOrganizationRequest): Promise<Organization> {
    return apiClient.post('/organizations', data);
  },

  /**
   * List all organizations the current user belongs to
   */
  async list(): Promise<Organization[]> {
    return apiClient.get('/my/organizations');
  },

  /**
   * Get organization by ID (requires X-Organization-Id header)
   */
  async get(organizationId: string): Promise<Organization> {
    return apiClient.get(`/organizations/${organizationId}`, {}, {
      headers: { 'X-Organization-Id': organizationId },
    });
  },

  /**
   * Update organization (requires org_admin)
   */
  async update(organizationId: string, data: UpdateOrganizationRequest): Promise<Organization> {
    return apiClient.patch('/organization', data, {
      headers: { 'X-Organization-Id': organizationId },
    });
  },

  /**
   * Delete organization (soft delete, requires org_admin)
   */
  async delete(organizationId: string): Promise<void> {
    console.log('[organizationService] delete called with:', organizationId);
    await apiClient.delete('/organization', {
      headers: { 'X-Organization-Id': organizationId },
    });
    console.log('[organizationService] delete completed');
  },

  /**
   * List organization members
   */
  async listMembers(organizationId: string): Promise<OrgMember[]> {
    return apiClient.get('/organization/members', {}, {
      headers: { 'X-Organization-Id': organizationId },
    });
  },

  /**
   * Add member to organization
   */
  async addMember(organizationId: string, data: AddOrgMemberRequest): Promise<OrgMember> {
    return apiClient.post('/organization/members', data, {
      headers: { 'X-Organization-Id': organizationId },
    });
  },

  /**
   * Remove member from organization
   */
  async removeMember(organizationId: string, userId: string): Promise<void> {
    await apiClient.delete(`/organization/members/${userId}`, {
      headers: { 'X-Organization-Id': organizationId },
    });
  },

  /**
   * Update member role
   */
  async updateMemberRole(organizationId: string, userId: string, role: 'org_admin' | 'org_user'): Promise<void> {
    await apiClient.post(`/organization/members/${userId}/role`, { role }, {
      headers: { 'X-Organization-Id': organizationId },
    });
  },
};
