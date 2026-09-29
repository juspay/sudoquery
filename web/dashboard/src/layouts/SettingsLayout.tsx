import { Box, Typography } from '@mui/material';
import { Outlet, NavLink, useLocation } from 'react-router-dom';
import { useOrganization } from '../contexts/OrganizationContext';
import {
  colorCream,
  colorCream2,
  colorInk,
  colorInk60,
  colorBlue,
  colorBluePale,
} from '../theme/tokens';

interface SettingsNavItemProps {
  to: string;
  label: string;
  adminOnly?: boolean;
  isAdmin: boolean;
}

function SettingsNavItem({ to, label, adminOnly, isAdmin }: SettingsNavItemProps) {
  const location = useLocation();
  const isActive = location.pathname === to;

  if (adminOnly && !isAdmin) {
    return null;
  }

  return (
    <NavLink
      to={to}
      style={{
        display: 'flex',
        alignItems: 'center',
        padding: '10px 16px',
        borderRadius: '6px',
        textDecoration: 'none',
        fontSize: '14px',
        fontWeight: 500,
        color: isActive ? colorBlue : colorInk60,
        backgroundColor: isActive ? colorBluePale : 'transparent',
        transition: 'all 0.15s',
      }}
    >
      {label}
    </NavLink>
  );
}

export default function SettingsLayout() {
  const { currentOrganization } = useOrganization();
  const isAdmin = currentOrganization?.access_level?.includes('admin') ?? false;

  return (
    <Box sx={{ display: 'flex', height: '100%', overflow: 'hidden' }}>
      {/* Settings Sidebar */}
      <Box
        sx={{
          width: 220,
          flexShrink: 0,
          height: '100%',
          bgcolor: colorCream,
          borderRight: '1px solid rgba(24,22,15,0.09)',
          display: 'flex',
          flexDirection: 'column',
          p: 2,
        }}
      >
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
          <SettingsNavItem
            to="/app/settings/account"
            label="Account"
            isAdmin={isAdmin}
          />
          <SettingsNavItem
            to="/app/settings/invitations"
            label="Invitations"
            isAdmin={isAdmin}
          />
          <SettingsNavItem
            to="/app/settings/project"
            label="Project"
            adminOnly
            isAdmin={isAdmin}
          />
          <SettingsNavItem
            to="/app/settings/organization"
            label="Organization"
            adminOnly
            isAdmin={isAdmin}
          />
        </Box>
      </Box>

      {/* Settings Content */}
      <Box
        sx={{
          flex: 1,
          overflow: 'auto',
          bgcolor: colorCream2,
        }}
      >
        <Outlet />
      </Box>
    </Box>
  );
}
