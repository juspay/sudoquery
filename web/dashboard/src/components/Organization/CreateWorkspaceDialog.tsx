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
import { useOrganization } from '../../contexts/OrganizationContext';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import type { Workspace } from '../../services/workspaceService';
import { slugError, slugify, workspaceId } from '../../utils/slug';
import { SlugInput } from '../shared/SlugInput';

interface CreateWorkspaceDialogProps {
  open: boolean;
  onClose: () => void;
  onSuccess: (workspace: Workspace) => void;
}

export function CreateWorkspaceDialog({ open, onClose, onSuccess }: CreateWorkspaceDialogProps) {
  const { currentOrganization } = useOrganization();
  const { createWorkspace } = useWorkspace();
  const [name, setName] = useState('');
  // Follows the name until edited.
  const [slug, setSlug] = useState('');
  const [slugEdited, setSlugEdited] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    if (!name.trim()) {
      setError('Workspace name is required');
      return;
    }
    const invalid = slugError(slug);
    if (invalid) {
      setError(invalid);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const workspace = await createWorkspace(name.trim(), slug);
      onSuccess(workspace);
      handleClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create workspace');
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    setName('');
    setSlug('');
    setSlugEdited(false);
    setError(null);
    onClose();
  };

  return (
    <Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
      <DialogTitle>Create Workspace</DialogTitle>
      <DialogContent>
        <Box sx={{ pt: 1 }}>
          {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}
          <TextField
            label="Workspace Name"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              if (!slugEdited) setSlug(slugify(e.target.value));
            }}
            fullWidth
            required
            placeholder="My Workspace"
            disabled={loading}
            onKeyDown={(e) => e.key === 'Enter' && handleSubmit()}
            autoFocus
          />
          <Box sx={{ mt: 2 }}>
            <SlugInput
              label="Workspace Slug"
              value={slug}
              onChange={(newSlug) => {
                setSlug(newSlug);
                setSlugEdited(true);
              }}
              idPreview={currentOrganization ? (s) => workspaceId(currentOrganization.id, s) : undefined}
              disabled={loading}
              onKeyDown={(e) => e.key === 'Enter' && handleSubmit()}
            />
          </Box>
        </Box>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={handleClose} disabled={loading}>
          Cancel
        </Button>
        <Button
          variant="contained"
          onClick={handleSubmit}
          disabled={loading || !name.trim() || !!slugError(slug)}
          startIcon={loading ? <CircularProgress size={16} /> : null}
        >
          Create
        </Button>
      </DialogActions>
    </Dialog>
  );
}
