import { Breadcrumbs, Link, Typography, Box } from '@mui/material';
import { Link as RouterLink, useLocation } from 'react-router-dom';
import { colorInk60 } from '../../theme/tokens';

const ROUTE_LABELS: Record<string, string> = {
  app: 'Home',
  chat: 'Chat',
  dashboard: 'Dashboard',
  metrics: 'Saved Metrics',
  settings: 'Settings',
  org: 'Organization',
  project: 'Project',
  members: 'Members',
  account: 'Account',
};

export function BreadcrumbBar() {
  const location = useLocation();
  const pathnames = location.pathname.split('/').filter((x) => x);

  // Don't show breadcrumbs on the main chat page to save vertical space
  if (location.pathname === '/app/chat') return null;

  return (
    <Box sx={{
      px: '24px',
      py: 1.5,
      bgcolor: 'var(--cream, #F8F6F0)',
      borderBottom: '1px solid rgba(24,22,15,0.09)'
    }}>
      <Breadcrumbs
        separator="›"
        aria-label="breadcrumb"
        sx={{
          fontSize: '11px',
          '& .MuiBreadcrumbs-separator': {
            color: '#C0BDB6',
            mx: 0.5,
          },
        }}
      >
        {pathnames.map((value, index) => {
          const isLast = index === pathnames.length - 1;
          const to = `/${pathnames.slice(0, index + 1).join('/')}`;
          const label = ROUTE_LABELS[value] || value;

          return isLast ? (
            <Typography
              key={to}
              color="text.primary"
              sx={{ fontSize: '11px', fontWeight: 500, color: '#18160F' }}
            >
              {label}
            </Typography>
          ) : (
            <Link
              key={to}
              component={RouterLink}
              to={to}
              underline="hover"
              sx={{ fontSize: '11px', color: '#6A6760', '&:hover': { color: '#18160F' } }}
            >
              {label}
            </Link>
          );
        })}
      </Breadcrumbs>
    </Box>
  );
}
