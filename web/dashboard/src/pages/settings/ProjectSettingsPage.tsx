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
import { ProjectSettingsForm } from '../../components/Settings/ProjectSettingsForm';
import { DangerZone } from '../../components/Settings/DangerZone';
import { MembersTable } from '../../components/Settings/MembersTable';
import type { MemberRow } from '../../components/Settings/MembersTable';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useProject } from '../../contexts/ProjectContext';
import { useToast } from '../../contexts/ToastContext';
import { projectService } from '../../services/projectService';
import { invitationService, type Invitation } from '../../services/invitationService';
import type { RoleType } from '../../components/Settings/RoleBadge';
import { trackEvent } from '../../utils/analytics';

export default function ProjectSettingsPage() {
  const { currentOrganization } = useOrganization();
  const { currentProject, setCurrentProject, projects, setProjects } = useProject();
  const navigate = useNavigate();
  const { showSuccess, showError } = useToast();
  const [isDeleting, setIsDeleting] = useState(false);

  // Members state
  const [projectMembers, setProjectMembers] = useState<MemberRow[]>([]);
  const [projectInvitations, setProjectInvitations] = useState<Invitation[]>([]);
  const [membersLoading, setMembersLoading] = useState(true);
  const [membersError, setMembersError] = useState<string | null>(null);

  // Invite dialog state
  const [inviteDialogOpen, setInviteDialogOpen] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('project_user');
  const [sendingInvite, setSendingInvite] = useState(false);

  // Delete confirmation dialog state
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);

  const isOrgAdmin = currentOrganization?.access_level?.includes('admin') ?? false;

  // Track page view
  useEffect(() => {
    trackEvent('page_view', { page: 'settings', tab: 'project' });
  }, []);

  useEffect(() => {
    if (!currentOrganization || !currentProject) {
      setMembersLoading(false);
      return;
    }

    const fetchMembers = async () => {
      setMembersLoading(true);
      setMembersError(null);

      try {
        const [projectMembersRes, projectInvitesRes] = await Promise.all([
          projectService.listMembers(currentOrganization.id, currentProject.id),
          invitationService.listProjectInvitations(currentOrganization.id, currentProject.id),
        ]);
        const projectRows: MemberRow[] = projectMembersRes.map((m) => ({
          id: m.user_id,
          name: m.username,
          email: m.email,
          role: m.role,
          joinedAt: new Date(),
        }));
        setProjectMembers(projectRows);
        setProjectInvitations(projectInvitesRes);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Failed to load members';
        setMembersError(message);
        showError(message);
      } finally {
        setMembersLoading(false);
      }
    };

    fetchMembers();
  }, [currentOrganization, currentProject, showError]);

  const handleRemoveProjectMember = async (id: string) => {
    if (!currentOrganization || !currentProject) return;

    try {
      await projectService.removeMember(currentOrganization.id, currentProject.id, id);
      setProjectMembers((prev) => prev.filter((m) => m.id !== id));
      showSuccess('Member removed from project.');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to remove member';
      showError(message);
    }
  };

  const handleUpdateProjectRole = async (id: string, newRole: RoleType) => {
    if (!currentOrganization || !currentProject) return;

    try {
      await projectService.updateMemberRole(currentOrganization.id, currentProject.id, id, newRole as 'project_admin' | 'project_user');
      setProjectMembers((prev) => prev.map((m) => (m.id === id ? { ...m, role: newRole } : m)));
      showSuccess('Project role updated successfully.');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to update role';
      showError(message);
    }
  };

  const handleSendInvite = async () => {
    if (!currentOrganization || !currentProject || !inviteEmail.trim()) return;

    setSendingInvite(true);
    try {
      await invitationService.inviteToProject(currentOrganization.id, currentProject.id, {
        email: inviteEmail.trim(),
        role: inviteRole,
      });
      showSuccess('Invitation sent successfully.');

      const invites = await invitationService.listProjectInvitations(currentOrganization.id, currentProject.id);
      setProjectInvitations(invites);

      setInviteDialogOpen(false);
      setInviteEmail('');
      setInviteRole('project_user');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to send invitation';
      showError(message);
    } finally {
      setSendingInvite(false);
    }
  };

  const handleRevokeInvitation = async (invitationId: string) => {
    if (!currentOrganization || !currentProject) return;
    try {
      await invitationService.revokeInvitation(invitationId);
      showSuccess('Invitation revoked.');
      const invites = await invitationService.listProjectInvitations(currentOrganization.id, currentProject.id);
      setProjectInvitations(invites);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to revoke invitation';
      showError(message);
    }
  };

  const formatRole = (role: string): string => {
    const roleMap: Record<string, string> = {
      org_admin: 'Organization Admin',
      org_user: 'Organization Member',
      project_admin: 'Project Admin',
      project_user: 'Project Member',
    };
    return roleMap[role] || role;
  };

  const closeDeleteDialog = () => {
    setDeleteDialogOpen(false);
  };

  const handleDeleteProject = () => {
    setDeleteDialogOpen(true);
  };

  const confirmDelete = async () => {
    if (!currentOrganization || !currentProject) {
      closeDeleteDialog();
      return;
    }

    setIsDeleting(true);
    try {
      await projectService.delete(currentOrganization.id, currentProject.id);
      showSuccess(`Project ${currentProject.name} deleted successfully.`);
      setProjects(projects.filter(p => p.id !== currentProject.id));
      setCurrentProject(null);
      closeDeleteDialog();
      navigate('/app/dashboard');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to delete project';
      showError(message);
    } finally {
      setIsDeleting(false);
    }
  };

  if (!currentProject) {
    return (
      <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 860, mx: 'auto' }}>
        <PageHeader
          title="Project Settings"
          description="Manage project settings and members."
        />
        <Alert severity="info">Select a project to view project settings.</Alert>
      </Box>
    );
  }

  return (
    <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 860, mx: 'auto' }}>
      <PageHeader
        title="Project Settings"
        description={`Manage settings for ${currentProject.name}.`}
      />

      <ProjectSettingsForm />

      {/* Members Section */}
      <Box sx={{ mt: 4 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
          <Typography variant="h6" fontWeight={600}>
            Members ({projectMembers.length})
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
              Project members have access only to this specific project.
            </Typography>
            <MembersTable
              members={projectMembers}
              isAdmin={isOrgAdmin}
              onUpdateRole={handleUpdateProjectRole}
              onRemoveMember={handleRemoveProjectMember}
              roleOptions={[
                { value: 'project_admin', label: 'Project Admin' },
                { value: 'project_user', label: 'Project Member' },
              ]}
              removeLabel="Remove from Project"
            />
            {projectInvitations.length > 0 && (
              <Box sx={{ mt: 4 }}>
                <Typography variant="subtitle2" sx={{ mb: 2, color: 'text.secondary' }}>
                  Pending Invitations ({projectInvitations.length})
                </Typography>
                <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
                  {projectInvitations.map((inv) => (
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
        title="Delete Project"
        description="Permanently delete this project and all of its associated analytics metrics. This action cannot be reversed."
        action={
          <Button variant="contained" color="error" onClick={handleDeleteProject} disabled={isDeleting}>
            Delete Project
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
          Delete Project
        </DialogTitle>
        <DialogContent>
          <DialogContentText sx={{ color: 'text.secondary', lineHeight: 1.6 }}>
            Are you sure you want to delete "{currentProject?.name}"? This will permanently delete this project and all of its associated analytics metrics. This action cannot be undone.
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
            To Project: {currentProject?.name}
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
                <MenuItem value="project_admin">Project Admin</MenuItem>
                <MenuItem value="project_user">Project User</MenuItem>
              </Select>
            </FormControl>
            <Typography variant="caption" color="text.secondary">
              Project members can only access this specific project.
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
