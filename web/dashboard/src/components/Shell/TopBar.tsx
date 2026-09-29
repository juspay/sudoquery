import { Box, AppBar, Toolbar, Divider, IconButton } from '@mui/material';
import { Menu as MenuIcon } from 'lucide-react';
import { Logo } from './Logo';
import { OrgProjectSwitcher } from './OrgProjectSwitcher';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useProject } from '../../contexts/ProjectContext';
import { useHistoryDrawer } from '../../contexts/HistoryDrawerContext';
import { projectService } from '../../services/projectService';
import { colorInk60, topbarHeight, fontFamilyBody } from '../../theme/tokens';
import { useEffect, useState } from 'react';

function EventsTodayBadge() {
  const { currentOrganization } = useOrganization();
  const { currentProject } = useProject();
  const [count, setCount] = useState<number | null>(null);

  useEffect(() => {
    if (!currentOrganization || !currentProject) {
      setCount(null);
      return;
    }

    let isActive = true;

    const loadCount = async () => {
      try {
        const nextCount = await projectService.getEventsTodayCount(currentOrganization.id, currentProject.id);
        if (isActive) {
          setCount(nextCount);
        }
      } catch {
        if (isActive) {
          setCount(null);
        }
      }
    };

    loadCount();
    const intervalId = window.setInterval(loadCount, 30_000);

    return () => {
      isActive = false;
      window.clearInterval(intervalId);
    };
  }, [currentOrganization?.id, currentProject?.id]);

  if (count === null) return null;

  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
      <Box
        sx={{
          width: 8,
          height: 8,
          borderRadius: '50%',
          bgcolor: '#4287f5',
          boxShadow: '0 0 8px rgba(66, 135, 245, 0.6), 0 0 12px rgba(66, 135, 245, 0.4)',
        }}
      />
      <Box sx={{ fontSize: '15px', color: colorInk60, fontFamily: fontFamilyBody, display: 'flex', alignItems: 'baseline', gap: 0.5 }}>
        <Box component="span" sx={{ fontWeight: 700, color: 'text.primary' }}>{count.toLocaleString()}</Box>
        <Box component="span">events today</Box>
      </Box>
    </Box>
  );
}

export function TopBar() {
  const { toggle: toggleHistoryDrawer } = useHistoryDrawer();

  return (
    <AppBar
      position="static"
      elevation={0}
      sx={{
        bgcolor: 'var(--cream, #F8F6F0)',
        borderBottom: `1px solid rgba(24,22,15,0.09)`,
        height: topbarHeight,
      }}
    >
      <Toolbar
        variant="dense"
        sx={{
          minHeight: topbarHeight,
          height: topbarHeight,
          px: { xs: '12px', md: '24px' },
          display: 'flex',
          alignItems: 'center',
          gap: { xs: 1, md: 2 },
          justifyContent: 'space-between',
        }}
      >
        {/* Mobile: Hamburger menu */}
        <IconButton
          onClick={toggleHistoryDrawer}
          sx={{ display: { xs: 'flex', md: 'none' }, mr: 1, p: 0.5 }}
        >
          <MenuIcon size={20} style={{ color: colorInk60 }} />
        </IconButton>

        {/* Left: Logo */}
        <Box sx={{ width: { xs: 'auto', md: 240 }, flexShrink: 0 }}>
          <Logo />
        </Box>

        {/* Center: Switcher */}
        <Box sx={{ flex: 1, display: 'flex', justifyContent: { xs: 'flex-start', md: 'center' } }}>
          <OrgProjectSwitcher />
        </Box>

        {/* Right: Actions & User */}
        <Box sx={{ width: { xs: 'auto', md: 240 }, display: 'flex', alignItems: 'center', gap: 2, flexShrink: 0, justifyContent: 'flex-end' }}>
          <Box sx={{ display: { xs: 'none', md: 'flex' } }}>
            <EventsTodayBadge />
          </Box>
          <Divider orientation="vertical" flexItem sx={{ display: { xs: 'none', md: 'block' }, my: 1.5 }} />
        </Box>
      </Toolbar>
    </AppBar>
  );
}
