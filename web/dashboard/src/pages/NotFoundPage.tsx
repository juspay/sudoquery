import { Box, Typography, Button, Container } from '@mui/material';
import { useNavigate } from 'react-router-dom';
import { colorCream, colorBlue, colorInk, fontFamilyDisplay } from '../theme/tokens';
import { Logo } from '../components/Shell/Logo';

export default function NotFoundPage() {
  const navigate = useNavigate();

  return (
    <Box sx={{ minHeight: '100vh', bgcolor: colorCream, display: 'flex', flexDirection: 'column' }}>
      <Box sx={{ p: 3, cursor: 'pointer' }} onClick={() => navigate('/')}>
        <Logo />
      </Box>

      <Container maxWidth="sm" sx={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center', textAlign: 'center', pb: 12 }}>
        <Typography
          sx={{
            fontFamily: fontFamilyDisplay,
            fontSize: '8rem',
            color: colorBlue,
            lineHeight: 1,
            mb: 2,
            opacity: 0.2,
          }}
        >
          404
        </Typography>

        <Typography variant="h3" sx={{ fontFamily: fontFamilyDisplay, color: colorInk, mb: 2 }}>
          Page not found
        </Typography>

        <Typography variant="body1" color="text.secondary" sx={{ mb: 4, maxWidth: 400 }}>
          Oops! It looks like you've wandered into uncharted territory. The page you're looking for doesn't exist or has been moved.
        </Typography>

        <Button
          variant="contained"
          size="large"
          onClick={() => navigate('/')}
          sx={{ px: 4 }}
        >
          Return Home
        </Button>
      </Container>
    </Box>
  );
}
