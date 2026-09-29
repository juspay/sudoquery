import { Box, Typography } from '@mui/material';
import { useLocation } from 'react-router-dom';
import { colorBlue, colorBluePale, colorInk40, colorCream3, fontFamilyBody } from '../../theme/tokens';

function IconCheck({ size = 10 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

const STEPS = [
  { label: 'Invitations', path: '/onboarding/invitations' },
  { label: 'Organization', path: '/onboarding/org' },
  { label: 'Project', path: '/onboarding/project' },
];

export function OnboardingProgressStepper() {
  const location = useLocation();
  const currentIndex = STEPS.findIndex((s) => location.pathname.startsWith(s.path));

  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 4, justifyContent: 'center' }}>
      {STEPS.map((step, i) => {
        const isCompleted = i < currentIndex;
        const isActive = i === currentIndex;

        return (
          <Box key={step.path} sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Box
              sx={{
                display: 'flex',
                alignItems: 'center',
                gap: 1,
                px: 2,
                py: 0.75,
                borderRadius: 20,
                bgcolor: isActive ? colorBlue : isCompleted ? colorBluePale : colorCream3,
                border: `1.5px solid ${isActive ? colorBlue : isCompleted ? colorBlue : colorCream3}`,
                transition: 'all 0.18s ease',
              }}
            >
              <Box
                sx={{
                  width: 18,
                  height: 18,
                  borderRadius: '50%',
                  bgcolor: isActive ? '#fff' : isCompleted ? colorBlue : colorInk40,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  fontSize: '0.65rem',
                  fontWeight: 700,
                  color: isCompleted ? '#fff' : isActive ? colorBlue : '#fff',
                  flexShrink: 0,
                }}
              >
                {isCompleted ? <IconCheck size={10} /> : i + 1}
              </Box>
              <Typography
                sx={{
                  fontFamily: fontFamilyBody,
                  fontSize: '0.8rem',
                  fontWeight: 600,
                  color: isActive ? '#fff' : isCompleted ? colorBlue : colorInk40,
                  whiteSpace: 'nowrap',
                }}
              >
                {step.label}
              </Typography>
            </Box>
            {i < STEPS.length - 1 && (
              <Box
                sx={{
                  width: 24,
                  height: 2,
                  borderRadius: 1,
                  bgcolor: isCompleted ? colorBlue : colorCream3,
                  transition: 'background-color 0.18s ease',
                }}
              />
            )}
          </Box>
        );
      })}
    </Box>
  );
}
