import { Box, Tooltip, Badge } from '@mui/material';
import { useEffect, useRef } from 'react';
import { NavLink, useLocation } from 'react-router-dom';
import { FileTextIcon } from '../icons/animated/FileTextIcon';
import { LayoutGridIcon } from '../icons/animated/LayoutGridIcon';
import { MessageCircleIcon } from '../icons/animated/MessageCircleIcon';
import { SettingsIcon } from '../icons/animated/SettingsIcon';
import { TerminalIcon } from '../icons/animated/TerminalIcon';
import type { AnimatedIconHandle } from '../icons/animated/types';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useSchemaStatus } from '../../contexts/SchemaContext';
import {
  colorCream,
  colorCream2,
  colorInk60,
  colorInk20,
  colorBluePale,
  colorBlue,
  colorRoseDark,
} from '../../theme/tokens';

interface NavItemProps {
  to: string;
  icon: React.ForwardRefExoticComponent<{
    size?: number;
    className?: string;
    onMouseEnter?: React.MouseEventHandler<HTMLDivElement>;
    onMouseLeave?: React.MouseEventHandler<HTMLDivElement>;
  } & React.RefAttributes<AnimatedIconHandle>>;
  label: string;
  adminOnly?: boolean;
  isAdmin: boolean;
  isActive: boolean;
  badgeCount?: number;
}

function NavItem({ to, icon, label, adminOnly, isAdmin, isActive, badgeCount }: NavItemProps) {
  const disabled = adminOnly && !isAdmin;
  const iconRef = useRef<AnimatedIconHandle>(null);
  const Icon = icon;

  const iconOnlyStyles: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '36px',
    height: '36px',
    borderRadius: '6px',
    textDecoration: 'none',
    cursor: disabled ? 'default' : 'pointer',
    border: 'none',
    background: isActive ? colorBluePale : 'transparent',
    color: disabled ? colorInk20 : isActive ? colorBlue : colorInk60,
    transition: 'all 0.15s',
  };

  const hoverStyles = disabled
    ? {}
    : {
        backgroundColor: isActive ? colorBluePale : colorCream2,
      };

  useEffect(() => {
    if (isActive) {
      void iconRef.current?.startAnimation();
      return;
    }

    void iconRef.current?.stopAnimation();
  }, [isActive]);

  if (disabled) {
    return (
      <Tooltip title={`${label} (Admin only)`} placement="right" arrow>
        <Box
          component="button"
          disabled
          sx={{
            ...iconOnlyStyles,
            '&:hover': hoverStyles,
            opacity: 0.5,
          }}
        >
          <Icon ref={iconRef} size={18} />
        </Box>
      </Tooltip>
    );
  }

  return (
    <Tooltip title={label} placement="right" arrow>
      <NavLink
        to={to}
        style={({ isActive: active }) => ({
          ...iconOnlyStyles,
          backgroundColor: active ? colorBluePale : 'transparent',
          color: active ? colorBlue : colorInk60,
        })}
      >
        <Badge
          variant="dot"
          color="error"
          invisible={!badgeCount}
          sx={{
            '& .MuiBadge-badge': {
              backgroundColor: colorRoseDark,
              minWidth: 8,
              height: 8,
              borderRadius: '50%',
              right: 2,
              top: 2,
            },
          }}
        >
          <Icon ref={iconRef} size={18} />
        </Badge>
      </NavLink>
    </Tooltip>
  );
}

export function SideNav() {
  const location = useLocation();
  const { currentOrganization } = useOrganization();
  const { eventsNeedingDescription, propertiesNeedingDescription } = useSchemaStatus();

  const isAdmin = currentOrganization?.access_level?.includes('admin') ?? false;
  const schemaBadgeCount = eventsNeedingDescription + propertiesNeedingDescription;

  return (
    <Box
      sx={{
        width: 56,
        flexShrink: 0,
        height: '100%',
        bgcolor: colorCream,
        borderRight: '1px solid rgba(24,22,15,0.09)',
        display: { xs: 'none', md: 'flex' },
        flexDirection: 'column',
        alignItems: 'center',
        py: 1.5,
        gap: '2px',
        overflowX: 'hidden',
        position: 'relative',
      }}
    >
      {/* Main Navigation Items */}
      <NavItem
        to="/app/chat"
        icon={MessageCircleIcon}
        label="Chat"
        isAdmin={isAdmin}
        isActive={location.pathname === '/app/chat'}
      />
      <NavItem
        to="/app/dashboard"
        icon={LayoutGridIcon}
        label="Dashboard"
        isAdmin={isAdmin}
        isActive={location.pathname === '/app/dashboard'}
      />
      <NavItem
        to="/app/sql-console"
        icon={TerminalIcon}
        label="SQL Console"
        isAdmin={isAdmin}
        isActive={location.pathname === '/app/sql-console'}
      />
      <NavItem
        to="/app/events-schema"
        icon={FileTextIcon}
        label="Events Schema"
        isAdmin={isAdmin}
        isActive={location.pathname === '/app/events-schema'}
        badgeCount={schemaBadgeCount}
      />
      <Box sx={{ flex: 1 }} />

      {/* Settings */}
      <NavItem
        to="/app/settings"
        icon={SettingsIcon}
        label="Settings"
        isAdmin={isAdmin}
        isActive={location.pathname.startsWith('/app/settings')}
      />
    </Box>
  );
}
