import { Box } from '@mui/material';
import { AnimatedMD } from '../Custom/AnimatedMD';
import { colorInk, colorInk80, fontFamilyBody } from '../../theme/tokens';

interface AssistantMessageBubbleProps {
  content: string;
}

export function AssistantMessageBubble({ content }: AssistantMessageBubbleProps) {
  return (
    <Box sx={{ mb: 1.5 }}>
      <Box
        sx={{
          bgcolor: 'transparent',
          color: colorInk80,
          borderRadius: '2px 10px 10px 10px',
          fontFamily: fontFamilyBody,
          fontSize: '15.4px',
          lineHeight: 1.55,
          maxWidth: '95%',
          wordBreak: 'break-word',
          '& strong': { color: colorInk, fontWeight: 600 },
          '& code': {
            bgcolor: 'rgba(0,0,0,0.06)',
            px: '4px',
            py: '2px',
            borderRadius: '4px',
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: '0.9em',
          },
          '& pre': {
            bgcolor: 'rgba(0,0,0,0.04)',
            p: 1,
            borderRadius: '6px',
            overflow: 'auto',
            '& code': { bgcolor: 'transparent', p: 0 },
          },
        }}
      >
        <div className="message-content">
          <AnimatedMD md={content} />
        </div>
      </Box>
    </Box>
  );
}
