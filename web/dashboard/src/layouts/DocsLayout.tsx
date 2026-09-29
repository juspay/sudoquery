import { Box, Typography } from '@mui/material';
import { Outlet, NavLink, useLocation, useNavigate } from 'react-router-dom';
import { Button } from '@mui/material';
import { Logo } from '../components/Shell/Logo';
import {
  colorCream,
  colorCream2,
  colorInk60,
  colorBlue,
  colorBluePale,
} from '../theme/tokens';

interface DocsNavItemProps {
  to: string;
  label: string;
}

function DocsNavItem({ to, label }: DocsNavItemProps) {
  const location = useLocation();
  const isActive = location.pathname === to;

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

export default function DocsLayout() {
  const navigate = useNavigate();

  return (
    <Box
      sx={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        bgcolor: colorCream,
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {/* Header */}
      <Box
        sx={{
          p: 3,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          borderBottom: '1px solid rgba(24,22,15,0.09)',
        }}
      >
        <Box sx={{ cursor: 'pointer' }} onClick={() => navigate('/')}>
          <Logo />
        </Box>
        <Box sx={{ display: 'flex', gap: 2 }}>
          <Button variant="outlined" onClick={() => navigate('/login')}>
            Sign In
          </Button>
        </Box>
      </Box>

      {/* Main Content */}
      <Box sx={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Sidebar */}
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
          <Typography
            sx={{
              fontSize: '12px',
              fontWeight: 600,
              letterSpacing: '0.08em',
              textTransform: 'uppercase',
              color: colorInk60,
              mb: 2,
              px: 1.5,
            }}
          >
            Documentation
          </Typography>

          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
            <DocsNavItem to="/docs/getting-started" label="Getting Started" />
            <DocsNavItem to="/docs/usage" label="Usage" />
            <DocsNavItem to="/docs/configuration" label="Configuration" />
            <DocsNavItem to="/docs/api-reference" label="API Reference" />
          </Box>
        </Box>

        {/* Content Area */}
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
    </Box>
  );
}
