import type { SxProps, Theme } from '@mui/material';
import { fontFamilyMono, radiusCard } from '../../theme/tokens';

/** The hairline used between surfaces across the app shell. */
export const HAIRLINE = '1px solid rgba(24,22,15,0.09)';

export const panelSx: SxProps<Theme> = {
  bgcolor: '#FFFFFF',
  border: HAIRLINE,
  borderRadius: `${radiusCard}px`,
};

/** Field names and values: compact monospace. */
export const monoSx = {
  fontFamily: fontFamilyMono,
  fontSize: '12.5px',
  lineHeight: 1.5,
} as const;

/** Small uppercase label above a group of controls or fields. */
export const overlineSx = {
  fontSize: '11px',
  fontWeight: 600,
  letterSpacing: '0.06em',
  textTransform: 'uppercase',
  color: 'text.secondary',
} as const;
