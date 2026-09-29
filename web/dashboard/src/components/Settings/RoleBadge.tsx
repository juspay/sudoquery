import { Chip, Box } from '@mui/material';
import { ShieldAlert, ShieldCheck, User } from 'lucide-react';
import { colorBluePale, colorBlueLight, colorRosePale, colorRoseDark, colorCream3, colorInk60 } from '../../theme/tokens';

export type RoleType = 'super_admin' | 'org_admin' | 'org_user' | 'project_admin' | 'project_user';

interface RoleBadgeProps {
  role: RoleType;
  size?: 'small' | 'medium';
}

const roleConfig: Record<RoleType, { label: string; bg: string; color: string; icon: React.ReactNode }> = {
  super_admin: { label: 'Super Admin', bg: colorRosePale, color: colorRoseDark, icon: <ShieldAlert size={14} /> },
  org_admin: { label: 'Org Admin', bg: colorBluePale, color: colorBlueLight, icon: <ShieldCheck size={14} /> },
  org_user: { label: 'Org Member', bg: colorCream3, color: colorInk60, icon: <User size={14} /> },
  project_admin: { label: 'Project Admin', bg: colorBluePale, color: colorBlueLight, icon: <ShieldCheck size={14} /> },
  project_user: { label: 'Project Member', bg: colorCream3, color: colorInk60, icon: <User size={14} /> },
};

export function RoleBadge({ role, size = 'small' }: RoleBadgeProps) {
  const config = roleConfig[role];
  if (!config) return null;

  return (
    <Chip
      size={size}
      label={
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5, fontWeight: 600 }}>
          {config.icon}
          <span>{config.label}</span>
        </Box>
      }
      sx={{
        bgcolor: config.bg,
        color: config.color,
        borderRadius: 1,
        height: size === 'small' ? 24 : 32,
        '& .MuiChip-label': {
          px: 1,
        },
      }}
    />
  );
}
