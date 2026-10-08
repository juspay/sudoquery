import { useState } from 'react';
import { Box, IconButton, Tooltip, Typography } from '@mui/material';
import { Check as CheckIcon, Copy as CopyIcon } from 'lucide-react';
import { fontFamilyMono } from '../../theme/tokens';

interface CopyableIdProps {
  /** Label shown above the value, e.g. "Project ID". */
  label: string;
  /** Read-only identifier (server-generated slug). Never editable. */
  value: string;
  /** Optional helper text shown below the value. */
  helperText?: string;
}

/**
 * Read-only identifier display with a copy-to-clipboard button.
 *
 * Ids are server-generated slugs (e.g. `acme-store-k3x9qa`) that users need
 * to copy for SDK/API configuration but can never type or edit — so this is
 * plain text, never an input field. Copy feedback follows the app-wide
 * Check/Copy icon swap.
 */
export function CopyableId({ label, value, helperText }: CopyableIdProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard unavailable (e.g. non-secure context) — nothing sensible to do
    }
  };

  return (
    <Box>
      <Typography variant="body2" sx={{ mb: 0.5 }}>
        {label}
      </Typography>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
        <Typography
          variant="body2"
          sx={{
            fontFamily: fontFamilyMono,
            fontSize: '0.875rem',
            wordBreak: 'break-all',
            flex: 1,
            color: 'text.primary',
          }}
        >
          {value}
        </Typography>
        <Tooltip title={copied ? 'Copied' : `Copy ${label}`}>
          <IconButton
            size="small"
            aria-label={`Copy ${label}`}
            onClick={handleCopy}
            sx={{ color: 'text.secondary' }}
          >
            {copied ? <CheckIcon size={16} /> : <CopyIcon size={16} />}
          </IconButton>
        </Tooltip>
      </Box>
      {helperText ? (
        <Typography variant="caption" color="text.secondary">
          {helperText}
        </Typography>
      ) : null}
    </Box>
  );
}
