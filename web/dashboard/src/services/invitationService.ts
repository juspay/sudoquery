import { apiClient } from './apiClient';

export interface Invitation {
  id: string;
  email: string;
  role: string;
  status: 'pending' | 'accepted' | 'revoked';
  created_at: string;
  expires_at?: string;
  organization_id?: string;
  organization_name?: string;
  workspace_id?: string | null;
  workspace_name?: string | null;
  invited_by?: {
    id: string;
    username: string;
    email: string;
  };
}

interface CreateInvitationRequest {
  email: string;
  role: string;
}

function orgHeader(organizationId: string): Record<string, string> {
  return { 'X-Organization-Id': organizationId };
}

export const invitationService = {
  // Invite to organization (OrgAdmin only)
  async inviteToOrg(organizationId: string, data: CreateInvitationRequest): Promise<Invitation> {
    return apiClient.post('/organization/invitations', data, {
      headers: orgHeader(organizationId),
    });
  },

  // Invite to workspace (WorkspaceAccess required)
  async inviteToWorkspace(organizationId: string, workspaceId: string, data: CreateInvitationRequest): Promise<Invitation> {
    return apiClient.post('/workspace/invitations', data, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  // List pending invitations for me
  async listMyInvitations(): Promise<Invitation[]> {
    return apiClient.get('/invitations/my', {}, {});
  },

  // List invitations sent by me
  async listSentInvitations(): Promise<Invitation[]> {
    return apiClient.get('/invitations/sent', {}, {});
  },

  // List pending org invitations (OrgContext required)
  async listOrgInvitations(organizationId: string): Promise<Invitation[]> {
    return apiClient.get('/organization/invitations', {}, {
      headers: orgHeader(organizationId),
    });
  },

  // List pending workspace invitations (WorkspaceAccess required)
  async listWorkspaceInvitations(organizationId: string, workspaceId: string): Promise<Invitation[]> {
    return apiClient.get('/workspace/invitations', {}, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  // Accept invitation
  async acceptInvitation(invitationId: string): Promise<void> {
    await apiClient.post(`/invitations/${invitationId}/accept`, {}, {});
  },

  // Revoke invitation (Inviter only)
  async revokeInvitation(invitationId: string): Promise<void> {
    await apiClient.post(`/invitations/${invitationId}/revoke`, {}, {});
  },
};
