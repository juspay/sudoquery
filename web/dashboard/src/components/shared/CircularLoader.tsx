import { Box } from '@mui/material';

interface CircularLoaderProps {
  size?: number;
  thickness?: number;
}

export function CircularLoader({ size = 40, thickness = 3 }: CircularLoaderProps) {
  return (
    <Box
      sx={{
        width: size,
        height: size,
        position: 'relative',
      }}
    >
      <Box
        sx={{
          width: size,
          height: size,
          borderRadius: '50%',
          border: `${thickness}px solid`,
          borderColor: 'divider',
        }}
      />
      <Box
        sx={{
          position: 'absolute',
          top: 0,
          left: 0,
          width: size,
          height: size,
          borderRadius: '50%',
          border: `${thickness}px solid transparent`,
          borderTopColor: 'primary.main',
          borderRightColor: 'primary.main',
          animation: 'spin 0.8s linear infinite',
          '@keyframes spin': {
            '0%': { transform: 'rotate(0deg)' },
            '100%': { transform: 'rotate(360deg)' },
          },
        }}
      />
    </Box>
  );
}
