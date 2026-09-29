import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, Button, TextField, Alert, Paper } from '@mui/material';
import { useOrganization } from '../contexts/OrganizationContext';
import { useProject } from '../contexts/ProjectContext';
import { OnboardingProgressStepper } from '../components/Onboarding/OnboardingProgressStepper';
import { fontFamilyDisplay } from '../theme/tokens';

export default function OnboardingProjectPage() {
  const navigate = useNavigate();
  const { currentOrganization } = useOrganization();
  const { createProject } = useProject();

  const [name, setName] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!name.trim()) {
      setError('Please enter a project name.');
      return;
    }
    if (!currentOrganization) {
      setError('Organization not found. Please try again.');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await createProject(name.trim());
      navigate('/app/chat', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create project.');
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && name.trim()) {
      handleSubmit();
    }
  };

  return (
    <Box>
      <OnboardingProgressStepper />
      <Paper
        elevation={0}
        sx={{ p: { xs: 3, sm: 5 }, borderRadius: 2, border: '1px solid', borderColor: 'divider', bgcolor: '#fff' }}
      >
        <Typography variant="h3" sx={{ fontFamily: fontFamilyDisplay, fontWeight: 700, mb: 0.75 }}>
          Create your first project
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3.5 }}>
          Projects contain your analytics configurations, queries, and team access rules.
        </Typography>

        {error && (
          <Alert severity="error" sx={{ mb: 2.5, borderRadius: 1 }}>{error}</Alert>
        )}

        <TextField
          label="Project name"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setError(null);
          }}
          onKeyDown={handleKeyDown}
          disabled={loading}
          fullWidth
          autoFocus
          placeholder="e.g., Marketing Analytics, Production Core"
          helperText="You can create more projects later."
          inputProps={{ maxLength: 60 }}
        />

        <Button
          variant="contained"
          fullWidth
          size="large"
          onClick={handleSubmit}
          disabled={!name.trim() || loading}
          sx={{ mt: 3, py: 1.5 }}
        >
          {loading ? 'Creating…' : 'Continue'}
        </Button>
      </Paper>
    </Box>
  );
}
