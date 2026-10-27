import { Box } from '@mui/material';
import { Outlet, useLocation } from 'react-router-dom';
import { TopBar } from '../components/Shell/TopBar';
import { SideNav } from '../components/Shell/SideNav';
import { BottomTabBar } from '../components/Shell/BottomTabBar';
import { BreadcrumbBar } from '../components/Shell/BreadcrumbBar';
import { WorkspaceSetupState } from '../components/Shell/WorkspaceStatus';
import { HistoryDrawerProvider } from '../contexts/HistoryDrawerContext';
import { useWorkspace } from '../contexts/WorkspaceContext';
import { colorCream } from '../theme/tokens';

export default function AppShell() {
  const { currentWorkspace } = useWorkspace();
  const { pathname } = useLocation();
  // Pages query the workspace, which only works once it is ACTIVE. Settings
  // stay reachable, so a workspace that is not ready can still be managed.
  const notReadyStatus =
    currentWorkspace && currentWorkspace.status !== 'ACTIVE' && !pathname.startsWith('/app/settings')
      ? currentWorkspace.status
      : null;

  return (
    <HistoryDrawerProvider>
      <Box sx={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden', bgcolor: colorCream }}>
        {/* Top Navigation */}
        <TopBar />

      <Box sx={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Side Navigation (Desktop via SideNav, Hidden on Mobile) */}
        <SideNav />

        {/* Main Content Area */}
        <Box
          component="main"
          sx={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            pb: { xs: 8, md: 0 }, // Add padding on mobile to account for BottomTabBar
          }}
        >
          {/* Breadcrumb Bar */}
          <BreadcrumbBar />

          {/* Nested Routes Render Here */}
          <Box sx={{ flex: 1, position: 'relative', overflow: 'auto', '&::-webkit-scrollbar': { display: 'none' }, msOverflowStyle: 'none', scrollbarWidth: 'none' }}>
            {notReadyStatus ? <WorkspaceSetupState status={notReadyStatus} /> : <Outlet />}
          </Box>
        </Box>
      </Box>

      {/* Bottom Navigation (Mobile Only) */}
      <BottomTabBar />
      </Box>
    </HistoryDrawerProvider>
  );
}
