import { Paper, BottomNavigation, BottomNavigationAction } from '@mui/material';
import { useEffect, useRef } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { ChartColumnIncreasingIcon } from '../icons/animated/ChartColumnIncreasingIcon';
import { LayoutGridIcon } from '../icons/animated/LayoutGridIcon';
import { MessageCircleIcon } from '../icons/animated/MessageCircleIcon';
import { SettingsIcon } from '../icons/animated/SettingsIcon';
import type { AnimatedIconHandle } from '../icons/animated/types';
import { colorCream2, colorBlue } from '../../theme/tokens';

function TabIcon({
  active,
  icon: Icon,
  size = 20,
}: {
  active: boolean;
  icon: React.ForwardRefExoticComponent<{
    size?: number;
    className?: string;
    onMouseEnter?: React.MouseEventHandler<HTMLDivElement>;
    onMouseLeave?: React.MouseEventHandler<HTMLDivElement>;
  } & React.RefAttributes<AnimatedIconHandle>>;
  size?: number;
}) {
  const iconRef = useRef<AnimatedIconHandle>(null);

  useEffect(() => {
    if (active) {
      void iconRef.current?.startAnimation();
      return;
    }

    void iconRef.current?.stopAnimation();
  }, [active]);

  return <Icon ref={iconRef} size={size} />;
}

export function BottomTabBar() {
  const navigate = useNavigate();
  const location = useLocation();

  // Highlight active tab based on route prefix
  const getActiveTab = () => {
    const path = location.pathname;
    if (path.startsWith('/app/chat')) return 'chat';
    if (path.startsWith('/app/dashboard')) return 'dashboard';
    if (path.startsWith('/app/metrics')) return 'metrics';
    if (path.startsWith('/app/settings')) return 'settings';
    return 'chat';
  };

  const currentTab = getActiveTab();

  const handleNav = (route: string) => {
    navigate(`/app/${route}`);
  };

  return (
    <Paper
      sx={{
        position: 'fixed',
        bottom: 0,
        left: 0,
        right: 0,
        display: { xs: 'block', md: 'none' }, // Visible on mobile, hidden on desktop
        borderTop: `1px solid ${colorCream2}`,
        zIndex: 1100,
      }}
      elevation={3}
    >
      <BottomNavigation
        showLabels
        value={currentTab}
        onChange={(_, newValue) => handleNav(newValue)}
        sx={{
          height: 64,
          '& .Mui-selected': {
            color: colorBlue,
          },
        }}
      >
        <BottomNavigationAction
          label="Chat"
          value="chat"
          icon={<TabIcon active={currentTab === 'chat'} icon={MessageCircleIcon} />}
        />
        <BottomNavigationAction
          label="Dashboard"
          value="dashboard"
          icon={<TabIcon active={currentTab === 'dashboard'} icon={LayoutGridIcon} />}
        />
        <BottomNavigationAction
          label="Metrics"
          value="metrics"
          icon={<TabIcon active={currentTab === 'metrics'} icon={ChartColumnIncreasingIcon} />}
        />
        <BottomNavigationAction
          label="Settings"
          value="settings"
          icon={<TabIcon active={currentTab === 'settings'} icon={SettingsIcon} />}
        />
      </BottomNavigation>
    </Paper>
  );
}
