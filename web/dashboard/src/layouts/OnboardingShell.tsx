import { Outlet } from 'react-router-dom';
import { Box, Container } from '@mui/material';
import { colorCream } from '../theme/tokens';

export default function OnboardingShell() {
  return (
    <Box
      sx={{
        minHeight: '100vh',
        bgcolor: colorCream,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'auto',
      }}
    >
      <Container maxWidth="sm" sx={{ py: 6 }}>
        <Outlet />
      </Container>
    </Box>
  );
}
