import { Box, Typography, Button, Switch, FormControlLabel } from '@mui/material';
import { PageHeader } from '../../components/shared/PageHeader';
import { DangerZone } from '../../components/Settings/DangerZone';
import { AvatarWithFallback } from '../../components/shared/AvatarWithFallback';
import { useAuth } from '../../contexts/AuthContext';
import { useDebugMode } from '../../hooks/useDebugMode';
import { colorCream2, colorInk, colorInk60 } from '../../theme/tokens';

export default function AccountSettingsPage() {
  const { user, logout } = useAuth();
  const { debugMode, setDebugMode } = useDebugMode();

  return (
    <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 860, mx: 'auto' }}>
      <PageHeader
        title="Account"
        description="Manage your personal profile and account settings."
      />

      <Box sx={{ p: { xs: 3, sm: 4 }, border: `1px solid ${colorCream2}`, borderRadius: 2, bgcolor: '#fff' }}>
        <Typography variant="h6" color={colorInk} mb={3}>
          Profile Information
        </Typography>

        {user && (
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 3 }}>
            <AvatarWithFallback name={user.preferred_username} email={user.email} size={72} />
            <Box>
              <Typography variant="subtitle1" fontWeight={500}>
                {user.preferred_username || 'User'}
              </Typography>
              <Typography variant="body2" color="text.secondary">
                {user.email || 'No email'}
              </Typography>
            </Box>
          </Box>
        )}
      </Box>

      <Box sx={{ p: { xs: 3, sm: 4 }, border: `1px solid ${colorCream2}`, borderRadius: 2, bgcolor: '#fff', mt: 3 }}>
        <Typography variant="h6" color={colorInk} mb={1}>
          Developer
        </Typography>
        <Typography variant="body2" color={colorInk60} mb={2}>
          Enable debug tools to inspect chat messages and API interactions.
        </Typography>
        <FormControlLabel
          control={<Switch checked={debugMode} onChange={(e) => setDebugMode(e.target.checked)} />}
          label="Debug Mode"
        />
      </Box>

      <DangerZone
        title="Sign Out"
        description="Sign out of Sudoquery on this device."
        action={
          <Button variant="contained" color="error" onClick={logout}>
            Sign Out
          </Button>
        }
      />
    </Box>
  );
}
