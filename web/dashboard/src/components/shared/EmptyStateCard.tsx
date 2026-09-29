import { Box, Typography, Button } from '@mui/material';
import type { ReactNode, SvgIconProps, ComponentType } from 'react';

interface EmptyStateCardProps {
  Icon?: ComponentType<SvgIconProps>;
  heading: string;
  subtext?: string;
  ctaLabel?: string;
  onCta?: () => void;
}

export function EmptyStateCard({ Icon, heading, subtext, ctaLabel, onCta }: EmptyStateCardProps) {
  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        textAlign: 'center',
        py: 8,
        px: 4,
        gap: 2,
      }}
    >
      {Icon && (
        <Box
          sx={{
            width: 56,
            height: 56,
            borderRadius: '50%',
            bgcolor: 'rgba(66,135,245,0.08)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            mb: 1,
          }}
        >
          <Icon sx={{ fontSize: 28, color: 'primary.main' }} />
        </Box>
      )}
      <Typography variant="h5" fontWeight={600} color="text.primary">
        {heading}
      </Typography>
      {subtext && (
        <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 360 }}>
          {subtext}
        </Typography>
      )}
      {ctaLabel && onCta && (
        <Button variant="contained" onClick={onCta} sx={{ mt: 1 }}>
          {ctaLabel}
        </Button>
      )}
    </Box>
  );
}
