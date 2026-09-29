import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, Button, Alert, Paper } from '@mui/material';
import { useOrganization } from '../contexts/OrganizationContext';
import { OrgNameInput } from '../components/Onboarding/OrgNameInput';
import { OnboardingProgressStepper } from '../components/Onboarding/OnboardingProgressStepper';
import { fontFamilyDisplay } from '../theme/tokens';

export default function OnboardingOrgPage() {
  const navigate = useNavigate();
  const { createOrganization } = useOrganization();
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleChange = (newName: string, newSlug: string) => {
    setName(newName);
    setSlug(newSlug);
    setError(null);
  };

  const handleSubmit = async () => {
    if (!name.trim()) {
      setError('Please enter an organization name.');
      return;
    }
    // Auto-generate slug if empty
    const finalSlug = slug.trim() || name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    setLoading(true);
    setError(null);
    try {
      await createOrganization(name.trim(), finalSlug);
      navigate('/onboarding/project', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create organization. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  const canSubmit = name.trim().length > 0 && !loading;

  return (
    <Box>
      <OnboardingProgressStepper />
      <Paper
        elevation={0}
        sx={{ p: { xs: 3, sm: 5 }, borderRadius: 2, border: '1px solid', borderColor: 'divider', bgcolor: '#fff' }}
      >
        <Typography variant="h3" sx={{ fontFamily: fontFamilyDisplay, fontWeight: 700, mb: 0.75 }}>
          Create your organization
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3.5 }}>
          An organization groups your projects and team members. You can change the name later.
        </Typography>

        {error && (
          <Alert severity="error" sx={{ mb: 2.5, borderRadius: 1 }}>{error}</Alert>
        )}

        <OrgNameInput value={name} onChange={handleChange} disabled={loading} />

        <Button
          variant="contained"
          fullWidth
          size="large"
          onClick={handleSubmit}
          disabled={!canSubmit}
          sx={{ mt: 3, py: 1.5 }}
        >
          {loading ? 'Creating…' : 'Continue'}
        </Button>
      </Paper>
    </Box>
  );
}
