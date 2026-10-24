import { useState, useEffect } from 'react';
import {
  Box,
  Button,
  Typography,
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  CircularProgress,
  Chip,
  IconButton,
  Tooltip,
  Select,
  FormControl,
  InputLabel,
  MenuItem,
  TextField,
} from '@mui/material';
import { Plus as AddIcon, X as CloseIcon } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { PageHeader } from '../../components/shared/PageHeader';
import { WorkspaceSettingsForm } from '../../components/Settings/WorkspaceSettingsForm';
import { DangerZone } from '../../components/Settings/DangerZone';
import { MembersTable } from '../../components/Settings/MembersTable';
import type { MemberRow } from '../../components/Settings/MembersTable';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { useToast } from '../../contexts/ToastContext';
import { workspaceService } from '../../services/workspaceService';
import { apiErrorMessage } from '../../services/apiClient';
import { invitationService, type Invitation } from '../../services/invitationService';
import type { RoleType } from '../../components/Settings/RoleBadge';
import { trackEvent } from '../../utils/analytics';

export default function WorkspaceSettingsPage() {
  const { currentOrganization } = useOrganization();
  const { currentWorkspace, setCurrentWorkspace, workspaces, setWorkspaces, refreshWorkspaces } = useWorkspace();
  const navigate = useNavigate();
  const { showSuccess, showError } = useToast();
  const [isDeleting, setIsDeleting] = useState(false);

  // Members state
  const [workspaceMembers, setWorkspaceMembers] = useState<MemberRow[]>([]);
  const [workspaceInvitations, setWorkspaceInvitations] = useState<Invitation[]>([]);
  const [membersLoading, setMembersLoading] = useState(true);
  const [membersError, setMembersError] = useState<string | null>(null);

  // Invite dialog state
  const [inviteDialogOpen, setInviteDialogOpen] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('workspace_user');
  const [sendingInvite, setSendingInvite] = useState(false);

  // Delete confirmation dialog state
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);

  const isOrgAdmin = currentOrganization?.access_level?.includes('admin') ?? false;

  // Track page view
  useEffect(() => {
    trackEvent('page_view', { page: 'settings', tab: 'workspace' });
  }, []);

  useEffect(() => {
    if (!currentOrganization || !currentWorkspace) {
      setMembersLoading(false);
      return;
    }

    const fetchMembers = async () => {
      setMembersLoading(true);
      setMembersError(null);

      try {
        const [workspaceMembersRes, workspaceInvitesRes] = await Promise.all([
          workspaceService.listMembers(currentOrganization.id, currentWorkspace.id),
          invitationService.listWorkspaceInvitations(currentOrganization.id, currentWorkspace.id),
        ]);
        const workspaceRows: MemberRow[] = workspaceMembersRes.map((m) => ({
          id: m.user_id,
          name: m.username,
          email: m.email,
          role: m.role,
          joinedAt: new Date(),
        }));
        setWorkspaceMembers(workspaceRows);
        setWorkspaceInvitations(workspaceInvitesRes);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Failed to load members';
        setMembersError(message);
        showError(message);
      } finally {
        setMembersLoading(false);
      }
    };

    fetchMembers();
  }, [currentOrganization, currentWorkspace, showError]);

  const handleRemoveWorkspaceMember = async (id: string) => {
    if (!currentOrganization || !currentWorkspace) return;

    try {
      await workspaceService.removeMember(currentOrganization.id, currentWorkspace.id, id);
      setWorkspaceMembers((prev) => prev.filter((m) => m.id !== id));
      showSuccess('Member removed from workspace.');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to remove member';
      showError(message);
    }
  };

  const handleUpdateWorkspaceRole = async (id: string, newRole: RoleType) => {
    if (!currentOrganization || !currentWorkspace) return;

    try {
      await workspaceService.updateMemberRole(currentOrganization.id, currentWorkspace.id, id, newRole as 'workspace_admin' | 'workspace_user');
      setWorkspaceMembers((prev) => prev.map((m) => (m.id === id ? { ...m, role: newRole } : m)));
      showSuccess('Workspace role updated successfully.');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to update role';
      showError(message);
    }
  };

  const handleSendInvite = async () => {
    if (!currentOrganization || !currentWorkspace || !inviteEmail.trim()) return;

    setSendingInvite(true);
    try {
      await invitationService.inviteToWorkspace(currentOrganization.id, currentWorkspace.id, {
        email: inviteEmail.trim(),
        role: inviteRole,
      });
      showSuccess('Invitation sent successfully.');

      const invites = await invitationService.listWorkspaceInvitations(currentOrganization.id, currentWorkspace.id);
      setWorkspaceInvitations(invites);

      setInviteDialogOpen(false);
      setInviteEmail('');
      setInviteRole('workspace_user');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to send invitation';
      showError(message);
    } finally {
      setSendingInvite(false);
    }
  };

  const handleRevokeInvitation = async (invitationId: string) => {
    if (!currentOrganization || !currentWorkspace) return;
    try {
      await invitationService.revokeInvitation(invitationId);
      showSuccess('Invitation revoked.');
      const invites = await invitationService.listWorkspaceInvitations(currentOrganization.id, currentWorkspace.id);
      setWorkspaceInvitations(invites);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to revoke invitation';
      showError(message);
    }
  };

  const formatRole = (role: string): string => {
    const roleMap: Record<string, string> = {
      org_admin: 'Organization Admin',
      org_user: 'Organization Member',
      workspace_admin: 'Workspace Admin',
      workspace_user: 'Workspace Member',
    };
    return roleMap[role] || role;
  };

  const closeDeleteDialog = () => {
    setDeleteDialogOpen(false);
  };

  const handleDeleteWorkspace = () => {
    setDeleteDialogOpen(true);
  };

  const confirmDelete = async () => {
    if (!currentOrganization || !currentWorkspace) {
      closeDeleteDialog();
      return;
    }

    setIsDeleting(true);
    try {
      await workspaceService.delete(currentOrganization.id, currentWorkspace.id);
      showSuccess(`Workspace ${currentWorkspace.name} deleted successfully.`);
      setWorkspaces(workspaces.filter(p => p.id !== currentWorkspace.id));
      setCurrentWorkspace(null);
      closeDeleteDialog();
      navigate('/app/dashboard');
    } catch (err) {
      showError(apiErrorMessage(err, 'Failed to delete workspace'));
      // A deletion that failed half-way leaves the workspace DELETING.
      await refreshWorkspaces();
    } finally {
      setIsDeleting(false);
    }
  };

  if (!currentWorkspace) {
    return (
      <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 860, mx: 'auto' }}>
        <PageHeader
          title="Workspace Settings"
          description="Manage workspace settings and members."
        />
        <Alert severity="info">Select a workspace to view workspace settings.</Alert>
      </Box>
    );
  }

  return (
    <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 860, mx: 'auto' }}>
      <PageHeader
        title="Workspace Settings"
        description={`Manage settings for ${currentWorkspace.name}.`}
      />

      {currentWorkspace.status !== 'ACTIVE' && (
        <Alert severity="info" sx={{ mb: 3 }}>
          {currentWorkspace.status === 'DELETING'
            ? 'This workspace is being deleted. If deleting it failed, you can retry below.'
            : 'This workspace is not set up yet, so it cannot receive events or be queried. Events sent with its token before then are lost.'}
        </Alert>
      )}

      <WorkspaceSettingsForm />

      {/* Members Section */}
      <Box sx={{ mt: 4 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
          <Typography variant="h6" fontWeight={600}>
            Members ({workspaceMembers.length})
          </Typography>
          <Button
            variant="contained"
            size="small"
            startIcon={<AddIcon size={20} />}
            onClick={() => setInviteDialogOpen(true)}
          >
            Invite
          </Button>
        </Box>

        {membersLoading && (
          <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
            <CircularProgress size={24} />
          </Box>
        )}

        {membersError && (
          <Alert severity="error" sx={{ mb: 3 }}>
            {membersError}
          </Alert>
        )}

        {!membersLoading && !membersError && (
          <>
            <Typography variant="body2" sx={{ mb: 2, color: 'text.secondary' }}>
              Workspace members have access only to this specific workspace.
            </Typography>
            <MembersTable
              members={workspaceMembers}
              isAdmin={isOrgAdmin}
              onUpdateRole={handleUpdateWorkspaceRole}
              onRemoveMember={handleRemoveWorkspaceMember}
              roleOptions={[
                { value: 'workspace_admin', label: 'Workspace Admin' },
                { value: 'workspace_user', label: 'Workspace Member' },
              ]}
              removeLabel="Remove from Workspace"
            />
            {workspaceInvitations.length > 0 && (
              <Box sx={{ mt: 4 }}>
                <Typography variant="subtitle2" sx={{ mb: 2, color: 'text.secondary' }}>
                  Pending Invitations ({workspaceInvitations.length})
                </Typography>
                <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
                  {workspaceInvitations.map((inv) => (
                    <Box
                      key={inv.id}
                      sx={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        p: 2,
                        border: '1px solid',
                        borderColor: 'divider',
                        borderRadius: 1,
                        bgcolor: 'background.paper',
                      }}
                    >
                      <Box>
                        <Typography variant="body2" fontWeight={500}>
                          {inv.email}
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          Sent {new Date(inv.created_at).toLocaleDateString()}
                        </Typography>
                      </Box>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        <Chip
                          label={formatRole(inv.role)}
                          size="small"
                          color="primary"
                          variant="outlined"
                        />
                        <Chip
                          label={inv.status}
                          size="small"
                          color={inv.status === 'pending' ? 'warning' : 'default'}
                        />
                      </Box>
                      <Tooltip title="Revoke Invitation">
                        <IconButton
                          size="small"
                          onClick={() => handleRevokeInvitation(inv.id)}
                          sx={{ color: 'text.secondary' }}
                        >
                          <CloseIcon size={16} />
                        </IconButton>
                      </Tooltip>
                    </Box>
                  ))}
                </Box>
              </Box>
            )}
          </>
        )}
      </Box>

      <DangerZone
        title="Delete Workspace"
        description="Permanently delete this workspace and all of its associated analytics metrics. This action cannot be reversed."
        action={
          <Button variant="contained" color="error" onClick={handleDeleteWorkspace} disabled={isDeleting}>
            Delete Workspace
          </Button>
        }
      />

      {/* Delete Confirmation Dialog */}
      <Dialog
        open={deleteDialogOpen}
        onClose={closeDeleteDialog}
        PaperProps={{
          sx: {
            borderRadius: 2,
            maxWidth: 480,
          },
        }}
      >
        <DialogTitle sx={{ color: 'error.main', fontWeight: 600, pb: 1 }}>
          Delete Workspace
        </DialogTitle>
        <DialogContent>
          <DialogContentText sx={{ color: 'text.secondary', lineHeight: 1.6 }}>
            Are you sure you want to delete "{currentWorkspace?.name}"? This will permanently delete this workspace and all of its associated analytics metrics. This action cannot be undone.
          </DialogContentText>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2.5, gap: 1 }}>
          <Button onClick={closeDeleteDialog} color="inherit" variant="outlined">
            Cancel
          </Button>
          <Button
            onClick={confirmDelete}
            color="error"
            variant="contained"
            disabled={isDeleting}
            autoFocus
          >
            {isDeleting ? 'Deleting...' : 'Delete'}
          </Button>
        </DialogActions>
      </Dialog>

      {/* Invite Dialog */}
      <Dialog open={inviteDialogOpen} onClose={() => setInviteDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>
          Invite Member
          <Typography variant="caption" display="block" color="text.secondary" sx={{ mt: 0.5 }}>
            To Workspace: {currentWorkspace?.name}
          </Typography>
        </DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3, mt: 1 }}>
            <TextField
              label="Email Address"
              type="email"
              value={inviteEmail}
              onChange={(e) => setInviteEmail(e.target.value)}
              placeholder="Enter email address"
              fullWidth
              autoFocus
            />
            <FormControl fullWidth>
              <InputLabel id="role-select-label">Role</InputLabel>
              <Select
                labelId="role-select-label"
                value={inviteRole}
                label="Role"
                onChange={(e) => setInviteRole(e.target.value)}
              >
                <MenuItem value="workspace_admin">Workspace Admin</MenuItem>
                <MenuItem value="workspace_user">Workspace User</MenuItem>
              </Select>
            </FormControl>
            <Typography variant="caption" color="text.secondary">
              Workspace members can only access this specific workspace.
            </Typography>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setInviteDialogOpen(false)} disabled={sendingInvite}>
            Cancel
          </Button>
          <Button
            variant="contained"
            onClick={handleSendInvite}
            disabled={!inviteEmail.trim() || sendingInvite}
          >
            {sendingInvite ? 'Sending...' : 'Send Invitation'}
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
