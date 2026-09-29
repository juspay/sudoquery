import { useEffect, useState, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { authService } from '../../services/authService';
import { organizationService } from '../../services/organizationService';
import { Box, CircularProgress, Typography } from '@mui/material';
import { colorCream, colorBlue } from '../../theme/tokens';

interface OAuthCallbackProps {
  onAuthSuccess: () => void;
}

// Global flag to prevent double-processing across remounts
let currentProcessingCode: string | null = null;

export function OAuthCallback({ onAuthSuccess }: OAuthCallbackProps) {
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);
  const processedRef = useRef(false);

  useEffect(() => {
    if (processedRef.current) return;

    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    const authError = params.get('error');
    const authErrorDescription = params.get('error_description');

    if (authError) {
      setError(authErrorDescription || authError);
      return;
    }

    if (!code) {
      setError('No authorization code received');
      return;
    }

    if (currentProcessingCode === code) return;

    currentProcessingCode = code;
    processedRef.current = true;

    const exchangeCode = async () => {
      try {
        await authService.handleOAuthCallback(code);
        onAuthSuccess();

        // Fetch org list directly (don't rely on context timing post-auth)
        try {
          const orgs = await organizationService.list();
          if (orgs.length === 0) {
            navigate('/onboarding/invitations', { replace: true });
          } else {
            navigate('/app/chat', { replace: true });
          }
        } catch {
          // If org fetch fails, default to chat (existing user)
          navigate('/app/chat', { replace: true });
        }
      } catch (err) {
        currentProcessingCode = null;
        setError(err instanceof Error ? err.message : 'Authentication failed');
      }
    };

    exchangeCode();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (error) {
    return (
      <Box
        sx={{
          height: '100vh',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 2,
          bgcolor: colorCream,
        }}
      >
        <Typography variant="h5" color="error">Authentication Failed</Typography>
        <Typography color="text.secondary">{error}</Typography>
      </Box>
    );
  }

  return (
    <Box
      sx={{
        height: '100vh',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 2,
        bgcolor: colorCream,
      }}
    >
      <CircularProgress sx={{ color: colorBlue }} />
      <Typography color="text.secondary">Completing sign in…</Typography>
    </Box>
  );
}

