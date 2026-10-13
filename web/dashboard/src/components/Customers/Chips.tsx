import { Chip } from '@mui/material';
import type { ProfileType, SegmentStatus } from '../../services/cdpService';
import { colorBlue, colorBluePale, colorCream2, colorInk60 } from '../../theme/tokens';

function SmallChip({ label, highlighted }: { label: string; highlighted: boolean }) {
  return (
    <Chip
      size="small"
      label={label}
      sx={{
        height: 20,
        fontSize: '11px',
        fontWeight: 600,
        textTransform: 'capitalize',
        bgcolor: highlighted ? colorBluePale : colorCream2,
        color: highlighted ? colorBlue : colorInk60,
      }}
    />
  );
}

/** Customer (has an email or phone) or anonymous visitor. */
export function ProfileTypeChip({ type }: { type: ProfileType }) {
  return <SmallChip label={type === 'customer' ? 'Customer' : 'Visitor'} highlighted={type === 'customer'} />;
}

export function StatusChip({ status }: { status: SegmentStatus }) {
  return <SmallChip label={status} highlighted={status === 'active'} />;
}
