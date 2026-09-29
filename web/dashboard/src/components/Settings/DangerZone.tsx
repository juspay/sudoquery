import { useState } from 'react';
import { Box, Typography, Button } from '@mui/material';
import { AlertCircle, ChevronDown, ChevronUp } from 'lucide-react';
import { colorRose, colorRosePale, colorInk60 } from '../../theme/tokens';

interface DangerZoneProps {
  title: string;
  description: string;
  action: React.ReactNode;
}

export function DangerZone({ title, description, action }: DangerZoneProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <Box
      sx={{
        border: `1px solid ${colorRose}`,
        borderRadius: 2,
        overflow: 'hidden',
        mt: 4,
      }}
    >
      <Box
        sx={{
          p: '12px 16px',
          bgcolor: colorRosePale,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          cursor: 'pointer',
        }}
        onClick={() => setExpanded(!expanded)}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <AlertCircle size={18} color={colorRose} />
          <Typography sx={{ fontSize: '13px', fontWeight: 600, color: colorRose }}>
            Danger Zone
          </Typography>
        </Box>
        <Button
          size="small"
          type="button"
          sx={{
            minWidth: 0,
            p: 0.5,
            color: colorRose,
            '&:hover': { bgcolor: 'rgba(165, 64, 44, 0.1)' },
          }}
        >
          {expanded ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
        </Button>
      </Box>
      {expanded && (
        <Box
          sx={{
            p: 3,
            bgcolor: '#fff',
            display: 'flex',
            flexDirection: { xs: 'column', sm: 'row' },
            alignItems: { xs: 'flex-start', sm: 'center' },
            justifyContent: 'space-between',
            gap: 2,
          }}
        >
          <Box>
            <Typography variant="body1" fontWeight={600} gutterBottom>
              {title}
            </Typography>
            <Typography variant="body2" color={colorInk60}>
              {description}
            </Typography>
          </Box>
          <Box sx={{ flexShrink: 0 }}>{action}</Box>
        </Box>
      )}
    </Box>
  );
}
