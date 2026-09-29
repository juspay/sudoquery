import { useState } from 'react';
import { Box, Typography, Divider, Paper } from '@mui/material';
import { useAuth } from '../contexts/AuthContext';
import { GoogleOAuthButton } from '../components/Auth/GoogleOAuthButton';
import {
  colorCream,
  colorInk,
  colorInk60,
  colorInk40,
  colorBlue,
  fontFamilyDisplay,
  fontFamilyBody,
} from '../theme/tokens';

export default function LoginPage() {
  const { googleLogin } = useAuth();
  const [loading, setLoading] = useState(false);

  const handleGoogleLogin = () => {
    setLoading(true);
    googleLogin();
    // Loading state will persist until redirect happens
  };

  return (
    <Box
      sx={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        bgcolor: colorCream,
        px: 2,
        position: 'relative',
        // Subtle radial gradient for depth
        '&::before': {
          content: '""',
          position: 'absolute',
          inset: 0,
          background: 'radial-gradient(ellipse at 60% 20%, rgba(66,135,245,0.06) 0%, transparent 60%)',
          pointerEvents: 'none',
        },
      }}
    >
      <Paper
        elevation={0}
        sx={{
          maxWidth: 420,
          width: '100%',
          px: { xs: 3, sm: 5 },
          py: { xs: 4, sm: 6 },
          border: '1px solid',
          borderColor: 'rgba(28,25,23,0.1)',
          borderRadius: '8px',
          bgcolor: '#FFFFFF',
          boxShadow: '0 4px 24px rgba(28,25,23,0.06), 0 1px 4px rgba(28,25,23,0.04)',
          position: 'relative',
          zIndex: 1,
        }}
      >
        {/* Brand mark */}
        <Box sx={{ mb: 4, textAlign: 'center' }}>
          <Typography
            variant="h3"
            sx={{
              fontFamily: fontFamilyDisplay,
              fontWeight: 700,
              fontSize: '1.75rem',
              color: colorInk,
              mb: 0.5,
              lineHeight: 1.2,
              '& em': {
                fontStyle: 'italic',
                color: colorBlue,
              },
            }}
          >
            Sudo<em>query</em>
          </Typography>
          <Typography
            variant="body2"
            sx={{ color: colorInk60, fontFamily: fontFamilyBody, fontSize: '0.9rem' }}
          >
            AI-powered insights for your data
          </Typography>
        </Box>

        <Divider sx={{ mb: 3, borderColor: 'rgba(28,25,23,0.08)' }} />

        <Typography
          variant="body2"
          sx={{ color: colorInk40, fontSize: '0.8rem', fontWeight: 500, mb: 1.5, letterSpacing: '0.05em', textTransform: 'uppercase' }}
        >
          Sign in to continue
        </Typography>

        <GoogleOAuthButton loading={loading} onClick={handleGoogleLogin} />

        <Typography
          variant="caption"
          sx={{ display: 'block', textAlign: 'center', color: colorInk40, mt: 3, lineHeight: 1.6, fontSize: '0.75rem' }}
        >
          By signing in, you agree to our{' '}
          <Box component="span" sx={{ color: 'primary.main', cursor: 'pointer', textDecoration: 'underline' }}>
            Terms of Service
          </Box>{' '}
          and{' '}
          <Box component="span" sx={{ color: 'primary.main', cursor: 'pointer', textDecoration: 'underline' }}>
            Privacy Policy
          </Box>
          .
        </Typography>
      </Paper>
    </Box>
  );
}
