import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, Button, Chip, CircularProgress, Paper, Alert } from '@mui/material';
import { Check as CheckIcon, Loader2 } from 'lucide-react';
import { useOrganization } from '../contexts/OrganizationContext';
import { useProject } from '../contexts/ProjectContext';
import { invitationService, type Invitation } from '../services/invitationService';
import { OnboardingProgressStepper } from '../components/Onboarding/OnboardingProgressStepper';
import { fontFamilyDisplay } from '../theme/tokens';

export default function OnboardingInvitationsPage() {
  const navigate = useNavigate();
  const { refreshOrganizations } = useOrganization();
  const { refreshProjects } = useProject();
  const [invitations, setInvitations] = useState<Invitation[]>([]);
  const [loading, setLoading] = useState(true);
  const [acceptingId, setAcceptingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchInvitations = async () => {
      try {
        const invites = await invitationService.listMyInvitations();
        const pending = invites.filter((i) => i.status === 'pending');
        if (pending.length === 0) {
          navigate('/onboarding/org', { replace: true });
        } else {
          setInvitations(pending);
        }
      } catch (err) {
        console.error('Failed to load invitations:', err);
        setError(err instanceof Error ? err.message : 'Failed to load invitations');
      } finally {
        setLoading(false);
      }
    };

    fetchInvitations();
  }, [navigate]);

  const handleAcceptInvitation = async (invitationId: string) => {
    setAcceptingId(invitationId);
    setError(null);
    try {
      await invitationService.acceptInvitation(invitationId);
      await refreshOrganizations();
      await refreshProjects();
      navigate('/app/chat', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to accept invitation');
    } finally {
      setAcceptingId(null);
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

  const handleSkip = () => {
    navigate('/onboarding/org', { replace: true });
  };

  if (loading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: 300 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box>
      <OnboardingProgressStepper />
      <Paper
        elevation={0}
        sx={{ p: { xs: 3, sm: 5 }, borderRadius: 2, border: '1px solid', borderColor: 'divider', bgcolor: '#fff' }}
      >
        <Typography variant="h3" sx={{ fontFamily: fontFamilyDisplay, fontWeight: 700, mb: 0.75 }}>
          You have pending invitations
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3.5 }}>
          Accept an invitation to join an existing organization, or create your own.
        </Typography>

        {error && (
          <Alert severity="error" sx={{ mb: 2.5, borderRadius: 1 }}>{error}</Alert>
        )}

        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          {invitations.map((inv) => (
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
                flexWrap: { xs: 'wrap', sm: 'nowrap' },
                gap: { xs: 2, sm: 1 },
              }}
            >
              <Box sx={{ minWidth: 0 }}>
                <Typography variant="body2" fontWeight={500}>
                  {inv.invited_by?.email || inv.email}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  Invited {new Date(inv.created_at).toLocaleDateString()}
                </Typography>
              </Box>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexShrink: 0 }}>
                <Chip
                  label={formatRole(inv.role)}
                  size="small"
                  color="primary"
                  variant="outlined"
                />
                <Button
                  variant="contained"
                  size="small"
                  startIcon={acceptingId === inv.id ? <Loader2 className="animate-spin" size={16} /> : <CheckIcon size={20} />}
                  onClick={() => handleAcceptInvitation(inv.id)}
                  disabled={acceptingId === inv.id}
                >
                  Accept
                </Button>
              </Box>
            </Box>
          ))}
        </Box>

        <Box sx={{ mt: 4, textAlign: 'center' }}>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            Want to start fresh?
          </Typography>
          <Button variant="text" onClick={handleSkip}>
            Create a new organization
          </Button>
        </Box>
      </Paper>
    </Box>
  );
}
