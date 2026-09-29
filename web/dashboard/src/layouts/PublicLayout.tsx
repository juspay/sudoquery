import { Navigate, Outlet } from 'react-router-dom';
import { Box } from '@mui/material';
import { useAuth } from '../contexts/AuthContext';
import { colorCream } from '../theme/tokens';

export default function PublicLayout() {
  const { isAuthenticated, loading } = useAuth();

  // Redirect authenticated users to dashboard
  if (!loading && isAuthenticated) {
    return <Navigate to="/app/dashboard" replace />;
  }

  return (
    <Box sx={{ minHeight: '100vh', bgcolor: colorCream, overflow: 'auto' }}>
      <Outlet />
    </Box>
  );
}
