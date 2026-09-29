import { Box, Typography, Container } from '@mui/material';
import { HardHat as ConstructionIcon } from 'lucide-react';
import { colorCream, colorBlue, colorInk, fontFamilyDisplay, fontFamilyBody } from '../theme/tokens';

export default function MaintenanceModePage() {
  return (
    <Box sx={{ minHeight: '100vh', bgcolor: colorCream, display: 'flex', flexDirection: 'column' }}>
      <Box sx={{ p: 3 }}>
        <Typography
          sx={{
            fontFamily: fontFamilyDisplay,
            fontSize: '18px',
            fontWeight: 700,
            color: colorInk,
            letterSpacing: '-0.5px',
            display: 'flex',
            alignItems: 'center',
            flexShrink: 0,
            '& em': {
              color: colorBlue,
              fontStyle: 'italic',
            }
          }}
        >
          Sudo<em>query</em>
        </Typography>
      </Box>

      <Container
        maxWidth="sm"
        sx={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          alignItems: 'center',
          textAlign: 'center',
          pb: 12,
        }}
      >
        <ConstructionIcon
          size={96}
          sx={{
            color: colorBlue,
            opacity: 0.6,
            mb: 3,
          }}
        />

        <Typography
          variant="h3"
          sx={{
            fontFamily: fontFamilyDisplay,
            color: colorInk,
            mb: 2,
          }}
        >
          Under Maintenance
        </Typography>

        <Typography variant="body1" color="text.secondary" sx={{ mb: 3, maxWidth: 420, lineHeight: 1.7 }}>
          We're currently performing scheduled maintenance to improve your experience.
          The dashboard will be back online shortly.
        </Typography>

        <Box
          sx={{
            mt: 2,
            p: 3,
            bgcolor: 'rgba(66, 135, 245, 0.04)',
            borderRadius: 2,
            border: '1px solid',
            borderColor: 'rgba(66, 135, 245, 0.12)',
            maxWidth: 420,
          }}
        >
          <Typography
            variant="body2"
            sx={{
              fontFamily: fontFamilyBody,
              color: colorInk,
              fontWeight: 500,
              mb: 1.5,
            }}
          >
            What you need to know:
          </Typography>
          <Box component="ul" sx={{ m: 0, p: 0, pl: 2.5, textAlign: 'left' }}>
            <Typography component="li" variant="body2" color="text.secondary" sx={{ mb: 0.75 }}>
              All events are still being recorded normally
            </Typography>
            <Typography component="li" variant="body2" color="text.secondary" sx={{ mb: 0.75 }}>
              Your data is safe and intact
            </Typography>
            <Typography component="li" variant="body2" color="text.secondary">
              No action is required on your part
            </Typography>
          </Box>
        </Box>

        <Typography
          variant="caption"
          sx={{
            mt: 4,
            color: 'text.secondary',
            fontFamily: fontFamilyBody,
          }}
        >
          Thank you for your patience.
        </Typography>
      </Container>
    </Box>
  );
}
