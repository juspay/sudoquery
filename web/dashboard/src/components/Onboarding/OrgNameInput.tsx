import { useState, useCallback, useEffect } from 'react';
import { Box, TextField } from '@mui/material';

function toSlug(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .substring(0, 48);
}

interface OrgNameInputProps {
  value: string;
  onChange: (name: string, slug: string) => void;
  error?: string;
  disabled?: boolean;
}

export function OrgNameInput({ value, onChange, error, disabled }: OrgNameInputProps) {
  // Auto-generate slug internally, but don't show it in UI
  const slug = toSlug(value);

  // Notify parent of slug changes when name changes
  useEffect(() => {
    onChange(value, slug);
  }, []); // Initial call only

  const handleNameChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const name = e.target.value;
      const autoSlug = toSlug(name);
      onChange(name, autoSlug);
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
