import { Navigate } from 'react-router-dom';
import { useAuth } from '../contexts/AuthContext';
import { useOrganization } from '../contexts/OrganizationContext';
import { LoadingSkeleton } from '../components/shared/LoadingSkeleton';
import { Alert, Box, Button } from '@mui/material';
import type { ReactNode } from 'react';

interface RequireAuthProps {
  children: ReactNode;
}

export function RequireAuth({ children }: RequireAuthProps) {
  const { isAuthenticated, loading: authLoading } = useAuth();
  const { organizations, initialized: orgInitialized, error: orgError, refreshOrganizations } = useOrganization();

  console.log('[RequireAuth] Render:', { authLoading, isAuthenticated, orgInitialized, orgCount: organizations?.length });

  // Wait for auth to load first
  if (authLoading) {
    console.log('[RequireAuth] Waiting for auth...');
    return <LoadingSkeleton variant="page" />;
  }

  if (!isAuthenticated) {
    console.log('[RequireAuth] Not authenticated, redirect to login');
    return <Navigate to="/login" replace />;
  }

  // Now wait for organizations to initialize
  if (!orgInitialized) {
    console.log('[RequireAuth] Waiting for orgs...');
    return <LoadingSkeleton variant="page" />;
  }

  // Show error state if we couldn't load organizations
  if (orgError) {
    return (
      <Box sx={{ p: 4, maxWidth: 600, mx: 'auto', mt: 10 }}>
        <Alert
          severity="error"
          action={
            <Button color="inherit" size="small" onClick={refreshOrganizations}>
              Retry
            </Button>
          }
        >
          Failed to load organizations: {orgError}
        </Alert>
      </Box>
    );
  }

  // Only redirect to onboarding if we successfully loaded and have no orgs
  if (organizations.length === 0) {
    console.log('[RequireAuth] No orgs, redirect to onboarding');
    return <Navigate to="/onboarding/invitations" replace />;
  }

  console.log('[RequireAuth] Rendering children');
  return <>{children}</>;
}
