import { TextField } from '@mui/material';
import { slugError, SLUG_MAX_LENGTH, SLUG_RULES } from '../../utils/slug';

interface SlugInputProps {
  label: string;
  value: string;
  onChange: (slug: string) => void;
  /** What the slug becomes, shown under the field, e.g. the workspace's id. */
  idPreview?: (slug: string) => string;
  disabled?: boolean;
  onKeyDown?: (e: React.KeyboardEvent) => void;
}

/** A slug field: lowercase as typed, checked against the slug rules. */
export function SlugInput({ label, value, onChange, idPreview, disabled, onKeyDown }: SlugInputProps) {
  const error = value ? slugError(value) : null;
  const helper = error ?? (value && idPreview ? `ID: ${idPreview(value)}. ` : '') + `Permanent. ${SLUG_RULES}.`;

  return (
    <TextField
      label={label}
      value={value}
      onChange={(e) => onChange(e.target.value.toLowerCase())}
      onKeyDown={onKeyDown}
      error={!!error}
      helperText={helper}
      disabled={disabled}
      fullWidth
      required
      inputProps={{ maxLength: SLUG_MAX_LENGTH, spellCheck: false, autoCapitalize: 'none' }}
    />
  );
}
