import { Box, Skeleton } from '@mui/material';

interface LoadingSkeletonProps {
  variant?: 'page' | 'card' | 'list' | 'chat';
}

export function LoadingSkeleton({ variant = 'page' }: LoadingSkeletonProps) {
  if (variant === 'card') {
    return (
      <Box sx={{ p: 3, borderRadius: 1, border: '1px solid', borderColor: 'divider' }}>
        <Skeleton variant="text" width="60%" height={28} sx={{ mb: 1 }} />
        <Skeleton variant="text" width="40%" height={20} sx={{ mb: 2 }} />
        <Skeleton variant="rectangular" height={120} sx={{ borderRadius: 1 }} />
      </Box>
    );
  }

  if (variant === 'list') {
    return (
      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
        {[...Array(5)].map((_, i) => (
          <Box key={i} sx={{ display: 'flex', alignItems: 'center', gap: 2, py: 1 }}>
            <Skeleton variant="circular" width={32} height={32} />
            <Box sx={{ flex: 1 }}>
              <Skeleton variant="text" width="50%" height={20} />
              <Skeleton variant="text" width="30%" height={16} />
            </Box>
          </Box>
        ))}
      </Box>
    );
  }

  if (variant === 'chat') {
    return (
      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, p: 2 }}>
        {[...Array(4)].map((_, i) => (
          <Box
            key={i}
            sx={{
              display: 'flex',
              justifyContent: i % 2 === 0 ? 'flex-start' : 'flex-end',
              gap: 1.5,
            }}
          >
            {i % 2 === 0 && <Skeleton variant="circular" width={28} height={28} />}
            <Box sx={{ maxWidth: '60%' }}>
              <Skeleton variant="rounded" width={i % 2 === 0 ? 240 : 180} height={44} sx={{ borderRadius: i % 2 === 0 ? '2px 10px 10px 10px' : '10px 10px 2px 10px' }} />
            </Box>
          </Box>
        ))}
      </Box>
    );
  }

  // page variant
  return (
    <Box
      sx={{
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        bgcolor: 'background.default',
      }}
    >
      <Box sx={{ width: 48, height: 48, position: 'relative' }}>
        <Box
          sx={{
            width: 48,
            height: 48,
            borderRadius: '50%',
            border: '3px solid',
            borderColor: 'primary.main',
            borderTopColor: 'transparent',
            animation: 'spin 0.8s linear infinite',
            '@keyframes spin': {
              '0%': { transform: 'rotate(0deg)' },
              '100%': { transform: 'rotate(360deg)' },
            },
          }}
        />
      </Box>
    </Box>
  );
}
