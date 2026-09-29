import { useState, useEffect } from 'react';
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
  Typography,
} from '@mui/material';

interface DescriptionEditDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (description: string) => Promise<void>;
  title: string;
  subtitle?: string;
  initialValue: string;
}

export function DescriptionEditDialog({
  open,
  onClose,
  onSave,
  title,
  subtitle,
  initialValue,
}: DescriptionEditDialogProps) {
  const [description, setDescription] = useState(initialValue);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (open) {
      setDescription(initialValue);
      setError(null);
    }
  }, [open, initialValue]);

  const handleSubmit = async () => {
    setLoading(true);
    setError(null);

    try {
      await onSave(description.trim());
      handleClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save description');
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    setDescription(initialValue);
    setError(null);
    onClose();
  };

  return (
    <Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
      <DialogTitle>
        <Box>
          {title}
          {subtitle && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
              {subtitle}
            </Typography>
          )}
        </Box>
      </DialogTitle>
      <DialogContent>
        <Box sx={{ pt: 1 }}>
          {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}
          <TextField
            label="Description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            fullWidth
            multiline
            rows={4}
            placeholder="Enter a description..."
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
          disabled={loading}
          startIcon={loading ? <CircularProgress size={16} /> : null}
        >
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}
