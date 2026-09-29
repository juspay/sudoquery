import { useState } from 'react';
import { Box, Typography, TextField, Button, Alert } from '@mui/material';
import { useOrganization } from '../../contexts/OrganizationContext';
import { organizationService } from '../../services/organizationService';
import { useToast } from '../../contexts/ToastContext';
import { colorInk, colorCream2 } from '../../theme/tokens';

export function OrgSettingsForm() {
  const { currentOrganization, setCurrentOrganization, refreshOrganizations } = useOrganization();
  const { showSuccess } = useToast();

  const [name, setName] = useState(currentOrganization?.name || '');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!currentOrganization) return null;

  const handleSave = async () => {
    if (!name.trim() || name === currentOrganization.name) return;

    setLoading(true);
    setError(null);

    try {
      const updated = await organizationService.update(currentOrganization.id, { name: name.trim() });
      setCurrentOrganization(updated);
      await refreshOrganizations();
      showSuccess('Organization updated successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update organization');
    } finally {
      setLoading(false);
    }
  };

  const isDirty = name.trim() !== currentOrganization.name;

  return (
    <Box sx={{ p: { xs: 3, sm: 4 }, border: `1px solid ${colorCream2}`, borderRadius: 2, bgcolor: '#fff' }}>
      <Typography variant="h6" color={colorInk} mb={3}>
        General Information
      </Typography>

      {error && <Alert severity="error" sx={{ mb: 3 }}>{error}</Alert>}

      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3, maxWidth: 480 }}>
        <TextField
          label="Organization Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={loading}
          fullWidth
          helperText="This is your company or team name."
        />

        <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 2 }}>
          <Button
            variant="contained"
            onClick={handleSave}
            disabled={!isDirty || loading || !name.trim()}
          >
            {loading ? 'Saving…' : 'Save Changes'}
          </Button>
        </Box>
      </Box>
    </Box>
  );
}
