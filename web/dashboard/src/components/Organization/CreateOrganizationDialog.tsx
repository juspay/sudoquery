import { useState } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  TextField,
  Box,
  Alert,
  CircularProgress,
} from '@mui/material';
import { organizationService, type Organization } from '../../services/organizationService';

interface CreateOrganizationDialogProps {
  open: boolean;
  onClose: () => void;
  onSuccess: (org: Organization) => void;
}

export function CreateOrganizationDialog({ open, onClose, onSuccess }: CreateOrganizationDialogProps) {
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleNameChange = (value: string) => {
    setName(value);
  };

  const handleSubmit = async () => {
    if (!name.trim()) {
      setError('Organization name is required');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const org = await organizationService.create({ name: name.trim() });
      onSuccess(org);
      handleClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create organization');
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    setName('');
    setError(null);
    onClose();
  };

  return (
    <Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
      <DialogTitle>Create Organization</DialogTitle>
      <DialogContent>
        <Box sx={{ pt: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {error && <Alert severity="error">{error}</Alert>}

          <TextField
            label="Organization Name"
            value={name}
            onChange={(e) => handleNameChange(e.target.value)}
            fullWidth
            required
            placeholder="My Company"
            disabled={loading}
            autoFocus
          />
        </Box>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={handleClose} disabled={loading}>
          Cancel
        </Button>
        <Button
          variant="contained"
          onClick={handleSubmit}
          disabled={loading || !name.trim()}
          startIcon={loading ? <CircularProgress size={16} /> : null}
        >
          Create
        </Button>
      </DialogActions>
    </Dialog>
  );
}
