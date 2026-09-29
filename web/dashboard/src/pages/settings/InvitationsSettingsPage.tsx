import { useEffect, useState, type ReactNode } from 'react';
import { Alert, Box, Button, Chip, CircularProgress, Tab, Tabs, Typography } from '@mui/material';
import { Check as CheckIcon, Loader2, Undo2 } from 'lucide-react';
import { PageHeader } from '../../components/shared/PageHeader';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useProject } from '../../contexts/ProjectContext';
import { useToast } from '../../contexts/ToastContext';
import { invitationService, type Invitation } from '../../services/invitationService';
import { trackEvent } from '../../utils/analytics';
import { colorCream2, colorInk } from '../../theme/tokens';

function formatRole(role: string): string {
  const roleMap: Record<string, string> = {
    org_admin: 'Organization Admin',
    org_user: 'Organization Member',
    project_admin: 'Project Admin',
    project_user: 'Project Member',
  };

  return roleMap[role] || role;
}

function getStatusBadgeStyles(status: Invitation['status']) {
  if (status === 'pending') {
    return {
      bgcolor: 'rgba(217, 119, 6, 0.12)',
      color: '#B45309',
    };
  }

  if (status === 'accepted') {
    return {
      bgcolor: 'rgba(22, 163, 74, 0.12)',
      color: '#15803D',
    };
  }

  return {
    bgcolor: 'rgba(100, 116, 139, 0.12)',
    color: '#475569',
  };
}

function getInvitationScope(invitation: Invitation) {
  const organizationName = invitation.organization_name?.trim();
  const projectName = invitation.project_name?.trim();

  if (organizationName && projectName) {
    return {
      primary: projectName,
      secondary: organizationName,
    };
  }

  if (organizationName) {
    return {
      primary: organizationName,
      secondary: 'Organization invitation',
    };
  }

  if (projectName) {
    return {
      primary: projectName,
      secondary: 'Project invitation',
    };
  }

  return {
    primary: invitation.role.startsWith('project_') ? 'Project invitation' : 'Organization invitation',
    secondary: undefined,
  };
}

function InvitationCard({
  invitation,
  action,
  showRecipientEmail = true,
}: {
  invitation: Invitation;
  action?: ReactNode;
  showRecipientEmail?: boolean;
}) {
  const scope = getInvitationScope(invitation);

  return (
    <Box
      sx={{
        display: 'grid',
        gridTemplateColumns: { xs: '1fr', md: 'minmax(0, 1fr) auto' },
        alignItems: 'center',
        gap: { xs: 1.5, md: 2 },
        p: { xs: 2, sm: 2.5 },
        border: '1px solid',
        borderColor: 'rgba(24,22,15,0.08)',
        borderRadius: 2,
        bgcolor: '#FCFBF8',
        boxShadow: '0 1px 0 rgba(24,22,15,0.03)',
      }}
    >
      <Box sx={{ minWidth: 0 }}>
        <Typography
          variant="body1"
          fontWeight={700}
          sx={{ wordBreak: 'break-word', color: colorInk, letterSpacing: '-0.01em' }}
        >
          {showRecipientEmail ? invitation.email : scope.primary}
        </Typography>
        <Box
          sx={{
            mt: 0.75,
            display: 'flex',
            flexWrap: 'wrap',
            gap: 1.25,
          }}
        >
          {!showRecipientEmail && scope.secondary && (
            <Typography variant="caption" color="text.secondary">
              {scope.secondary}
            </Typography>
          )}
          {showRecipientEmail && (
            <Typography variant="caption" color="text.secondary">
              {scope.primary}
              {scope.secondary ? ` • ${scope.secondary}` : ''}
            </Typography>
          )}
          <Typography variant="caption" color="text.secondary">
            Sent {new Date(invitation.created_at).toLocaleDateString()}
          </Typography>
          {invitation.invited_by?.email && (
            <Typography variant="caption" color="text.secondary">
              Invited by {invitation.invited_by.email}
            </Typography>
          )}
          {invitation.expires_at && (
            <Typography variant="caption" color="text.secondary">
              Expires {new Date(invitation.expires_at).toLocaleDateString()}
            </Typography>
          )}
        </Box>
      </Box>

      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 1.25,
          flexWrap: 'wrap',
          justifyContent: { xs: 'flex-start', md: 'flex-end' },
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
          <Chip
            label={formatRole(invitation.role)}
            size="small"
            variant="filled"
            sx={{
              height: 26,
              borderRadius: '8px',
              bgcolor: 'rgba(15, 23, 42, 0.06)',
              color: '#334155',
              '& .MuiChip-label': {
                px: 1.25,
                fontWeight: 600,
                fontSize: '0.74rem',
              },
            }}
          />
          <Chip
            label={invitation.status}
            size="small"
            variant="filled"
            sx={{
              height: 26,
              borderRadius: '999px',
              textTransform: 'capitalize',
              ...getStatusBadgeStyles(invitation.status),
              '& .MuiChip-label': {
                px: 1.15,
                fontWeight: 700,
                fontSize: '0.73rem',
              },
            }}
          />
        </Box>
        {action ? <Box sx={{ ml: { xs: 0, md: 0.75 } }}>{action}</Box> : null}
      </Box>
    </Box>
  );
}

export default function InvitationsSettingsPage() {
  const { refreshOrganizations } = useOrganization();
  const { refreshProjects } = useProject();
  const { showSuccess, showError } = useToast();
  const [activeTab, setActiveTab] = useState<'my' | 'sent'>('my');
  const [myInvitations, setMyInvitations] = useState<Invitation[]>([]);
  const [sentInvitations, setSentInvitations] = useState<Invitation[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingError, setLoadingError] = useState<string | null>(null);
  const [acceptingId, setAcceptingId] = useState<string | null>(null);
  const [revokingId, setRevokingId] = useState<string | null>(null);

  useEffect(() => {
    trackEvent('page_view', { page: 'settings', tab: 'invitations' });
  }, []);

  const loadInvitations = async () => {
    setLoading(true);
    setLoadingError(null);

    try {
      const [nextMyInvitations, nextSentInvitations] = await Promise.all([
        invitationService.listMyInvitations(),
        invitationService.listSentInvitations(),
      ]);

      setMyInvitations(nextMyInvitations.filter((invitation) => invitation.status === 'pending'));
      setSentInvitations(nextSentInvitations);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load invitations';
      setLoadingError(message);
      showError(message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadInvitations();
  }, []);

  const handleAcceptInvitation = async (invitationId: string) => {
    setAcceptingId(invitationId);

    try {
      await invitationService.acceptInvitation(invitationId);
      showSuccess('Invitation accepted successfully.');
      setMyInvitations((prev) => prev.filter((invitation) => invitation.id !== invitationId));
      await refreshOrganizations();
      await refreshProjects();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to accept invitation';
      showError(message);
    } finally {
      setAcceptingId(null);
    }
  };

  const handleRevokeInvitation = async (invitationId: string) => {
    setRevokingId(invitationId);

    try {
      await invitationService.revokeInvitation(invitationId);
      showSuccess('Invitation revoked.');
      setSentInvitations((prev) =>
        prev.map((invitation) =>
          invitation.id === invitationId ? { ...invitation, status: 'revoked' } : invitation,
        ),
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to revoke invitation';
      showError(message);
    } finally {
      setRevokingId(null);
    }
  };

  return (
    <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 840, mx: 'auto' }}>
      <PageHeader
        title="Invitations"
        description="Review invitations sent to you and invitations you have sent."
      />

      {loadingError && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {loadingError}
        </Alert>
      )}

      <Box
        sx={{
          border: `1px solid ${colorCream2}`,
          borderRadius: 3,
          bgcolor: '#fff',
          overflow: 'hidden',
          boxShadow: '0 12px 30px rgba(24,22,15,0.05)',
        }}
      >
        <Tabs
          value={activeTab}
          onChange={(_, value: 'my' | 'sent') => setActiveTab(value)}
          sx={{
            px: { xs: 1.5, sm: 2.5 },
            pt: 1.5,
            borderBottom: `1px solid ${colorCream2}`,
            bgcolor: 'rgba(255,255,255,0.78)',
            '& .MuiTab-root': {
              textTransform: 'none',
              minHeight: 48,
              fontWeight: 600,
              fontSize: '0.98rem',
              px: { xs: 1.5, sm: 2 },
            },
          }}
        >
          <Tab label={`My Invitations (${myInvitations.length})`} value="my" />
          <Tab label={`Sent By Me (${sentInvitations.length})`} value="sent" />
        </Tabs>

        <Box sx={{ p: { xs: 2.5, sm: 3.5 } }}>
          {activeTab === 'my' ? (
            <>
              <Typography variant="h6" color={colorInk} mb={0.75}>
                My Invitations
              </Typography>
              <Typography variant="body2" color="text.secondary" mb={2.5}>
                Accept pending invitations that were sent to your account.
              </Typography>

              {loading ? (
                <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
                  <CircularProgress size={24} />
                </Box>
              ) : myInvitations.length === 0 ? (
                <Typography variant="body2" color="text.secondary">
                  No pending invitations.
                </Typography>
              ) : (
                <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                  {myInvitations.map((invitation) => (
                    <InvitationCard
                      key={invitation.id}
                      invitation={invitation}
                      showRecipientEmail={false}
                      action={
                        <Button
                          variant="contained"
                          size="small"
                          startIcon={
                            acceptingId === invitation.id ? (
                              <Loader2 className="animate-spin" size={16} />
                            ) : (
                              <CheckIcon size={18} />
                            )
                          }
                          onClick={() => handleAcceptInvitation(invitation.id)}
                          disabled={acceptingId === invitation.id}
                        >
                          Accept
                        </Button>
                      }
                    />
                  ))}
                </Box>
              )}
            </>
          ) : (
            <>
              <Typography variant="h6" color={colorInk} mb={0.75}>
                Sent By Me
              </Typography>
              <Typography variant="body2" color="text.secondary" mb={2.5}>
                Track invitations you have sent and revoke pending ones when needed.
              </Typography>

              {loading ? (
                <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
                  <CircularProgress size={24} />
                </Box>
              ) : sentInvitations.length === 0 ? (
                <Typography variant="body2" color="text.secondary">
                  No invitations sent yet.
                </Typography>
              ) : (
                <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                  {sentInvitations.map((invitation) => (
                    <InvitationCard
                      key={invitation.id}
                      invitation={invitation}
                      action={
                        invitation.status === 'pending' ? (
                          <Button
                            variant="outlined"
                            size="small"
                            color="inherit"
                            startIcon={
                              revokingId === invitation.id ? (
                                <Loader2 className="animate-spin" size={16} />
                              ) : (
                                <Undo2 size={16} />
                              )
                            }
                            onClick={() => handleRevokeInvitation(invitation.id)}
                            disabled={revokingId === invitation.id}
                          >
                            Revoke
                          </Button>
                        ) : undefined
                      }
                    />
                  ))}
                </Box>
              )}
            </>
          )}
        </Box>
      </Box>
    </Box>
  );
}
