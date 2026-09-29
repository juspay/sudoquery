import { Box, Typography } from '@mui/material';
import { HardHat as ConstructionIcon } from 'lucide-react';

interface SavedMetricsProps {
  metrics: unknown[];
  onViewMetric: () => void;
  onDeleteMetric: () => void;
  onMetricCreated?: () => void;
}

export function SavedMetrics({
  metrics,
  onViewMetric,
  onDeleteMetric,
  onMetricCreated,
}: SavedMetricsProps) {
  return (
    <Box
      sx={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        color: 'text.secondary',
        p: 4,
        gap: 2,
      }}
    >
      <ConstructionIcon size={64} style={{ opacity: 0.4 }} />
      <Typography variant="h5" fontWeight={600} color="text.primary">
        Work in Progress
      </Typography>
      <Typography variant="body1" color="text.secondary">
        Saved metrics coming soon
      </Typography>
    </Box>
  );
}
