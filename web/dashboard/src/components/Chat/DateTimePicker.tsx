import { useState } from 'react';
import {
  Box,
  Typography,
  TextField,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  IconButton,
} from '@mui/material';
import { X as CloseIcon } from 'lucide-react';
import { colorInk, colorInk60, colorCream3, colorBlue } from '../../theme/tokens';

interface DateTimePickerProps {
  message: string;
  type: 'range' | 'single';
  onSubmit: (startDate: string, endDate?: string) => void;
  onCancel: () => void;
}

export function DateTimePicker({ message, type, onSubmit, onCancel }: DateTimePickerProps) {
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');

  const handleSubmit = () => {
    if (!startDate) return;
    if (type === 'range' && !endDate) return;

    onSubmit(startDate, type === 'range' ? endDate : undefined);
  };

  const isValid = type === 'range'
    ? startDate && endDate
    : startDate;

  return (
    <Dialog
      open
      onClose={onCancel}
      maxWidth="xs"
      fullWidth
      PaperProps={{
        sx: {
          borderRadius: 3,
          boxShadow: '0 8px 32px rgba(0,0,0,0.12)',
        },
      }}
    >
      <DialogTitle
        sx={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          pb: 1,
        }}
      >
        <Typography
          sx={{
            fontSize: '16px',
            fontWeight: 600,
            color: colorInk,
          }}
        >
          {message}
        </Typography>
        <IconButton
          size="small"
          onClick={onCancel}
          sx={{
            padding: 0.5,
            color: colorInk60,
            '&:hover': { bgcolor: 'transparent' },
          }}
        >
          <CloseIcon size={18} />
        </IconButton>
      </DialogTitle>

      <DialogContent sx={{ pt: 1 }}>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2.5, mt: 1 }}>
          <Box>
            <Typography
              sx={{
                fontSize: '12px',
                color: colorInk60,
                mb: 0.5,
                fontWeight: 500,
              }}
            >
              {type === 'range' ? 'Start Date' : 'Date'}
            </Typography>
            <TextField
              type="date"
              size="small"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              fullWidth
              InputLabelProps={{ shrink: true }}
              sx={{
                '& .MuiOutlinedInput-root': {
                  borderRadius: 2,
                  fontSize: '14px',
                },
              }}
            />
          </Box>

          {type === 'range' && (
            <Box>
              <Typography
                sx={{
                  fontSize: '12px',
                  color: colorInk60,
                  mb: 0.5,
                  fontWeight: 500,
                }}
              >
                End Date
              </Typography>
              <TextField
                type="date"
                size="small"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
                fullWidth
                InputLabelProps={{ shrink: true }}
                sx={{
                  '& .MuiOutlinedInput-root': {
                    borderRadius: 2,
                    fontSize: '14px',
                  },
                }}
              />
            </Box>
          )}
        </Box>
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 2.5, pt: 1 }}>
        <Button
          variant="text"
          onClick={onCancel}
          sx={{
            color: colorInk60,
            fontSize: '14px',
            fontWeight: 500,
            textTransform: 'none',
            '&:hover': { bgcolor: 'transparent' },
          }}
        >
          Cancel
        </Button>
        <Button
          variant="contained"
          onClick={handleSubmit}
          disabled={!isValid}
          sx={{
            bgcolor: colorBlue,
            color: '#fff',
            fontSize: '14px',
            fontWeight: 500,
            textTransform: 'none',
            borderRadius: 2,
            px: 3,
            '&:hover': {
              bgcolor: colorBlue,
              opacity: 0.9,
            },
            '&:disabled': {
              bgcolor: colorCream3,
              color: colorInk60,
            },
          }}
        >
          Apply
        </Button>
      </DialogActions>
    </Dialog>
  );
}
