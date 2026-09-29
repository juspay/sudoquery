import { Box, Typography } from '@mui/material';
import { fontFamilyDisplay, colorInk } from '../../theme/tokens';
import type { ReactNode } from 'react';

interface PageHeaderProps {
  title: string;
  description?: string;
  action?: ReactNode;
}

export function PageHeader({ title, description, action }: PageHeaderProps) {
  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'space-between',
        mb: description ? 3 : 0,
        gap: 2,
      }}
    >
      <Box>
        <Typography
          sx={{
            fontFamily: fontFamilyDisplay,
            fontSize: '28px',
            fontWeight: 400,
            color: colorInk,
            letterSpacing: '-0.5px',
            mb: description ? 1 : 0
          }}
        >
          {title}
        </Typography>
        {description && (
          <Typography sx={{ fontSize: '15px', color: '#6A6760', lineHeight: 1.6 }}>
            {description}
          </Typography>
        )}
      </Box>
      {action && <Box sx={{ flexShrink: 0 }}>{action}</Box>}
    </Box>
  );
}
