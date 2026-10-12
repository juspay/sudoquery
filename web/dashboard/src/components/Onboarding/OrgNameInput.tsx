import { useCallback } from 'react';
import { Box, TextField } from '@mui/material';

interface OrgNameInputProps {
  value: string;
  onChange: (name: string) => void;
  error?: string;
  disabled?: boolean;
}

export function OrgNameInput({ value, onChange, error, disabled }: OrgNameInputProps) {
  const handleNameChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      onChange(e.target.value);
    },
    [onChange],
  );

  return (
    <Box>
      <TextField
        label="Organization name"
        value={value}
        onChange={handleNameChange}
        error={!!error}
        helperText={error}
        disabled={disabled}
        fullWidth
        autoFocus
        placeholder="Acme Corp"
        inputProps={{ maxLength: 60 }}
      />
    </Box>
  );
}
