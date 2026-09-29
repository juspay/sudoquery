import { Box } from '@mui/material';
import { useState } from 'react';
import { colorInk, fontFamilyBody } from '../../theme/tokens';
import { Pencil as EditIcon, RotateCcw as RetryIcon, Copy as CopyIcon, Check} from 'lucide-react';

interface UserMessageBubbleProps {
  content: string;
  onRetry?: () => void;
  onEdit?: () => void;
}

export function UserMessageBubble({ content, onRetry, onEdit }: UserMessageBubbleProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      console.error('Failed to copy to clipboard');
    }
  };

  const [isHovered, setIsHovered] = useState(false);

  return (
    <Box
      sx={{
        display: 'flex',
        justifyContent: 'flex-end',
        alignItems: 'center',
        mb: 1.5,
        gap: '8px',
      }}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <Box
        sx={{
          display: 'flex',
          gap: '4px',
          opacity: isHovered ? 1 : 0,
          transition: 'opacity 0.15s',
          alignItems: 'center',
        }}
      >
        {onEdit && (
          <Box
            component="button"
            onClick={onEdit}
            sx={{
              color: '#928F88',
              bgcolor: 'transparent',
              border: 'none',
              cursor: 'pointer',
              padding: '6px',
              borderRadius: '4px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              '&:hover': {
                color: colorInk,
                bgcolor: 'rgba(0,0,0,0.04)',
              },
            }}
          >
            <EditIcon size={14} />
          </Box>
        )}
        {onRetry && (
          <Box
            component="button"
            onClick={onRetry}
            sx={{
              color: '#928F88',
              bgcolor: 'transparent',
              border: 'none',
              cursor: 'pointer',
              padding: '6px',
              borderRadius: '4px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              '&:hover': {
                color: colorInk,
                bgcolor: 'rgba(0,0,0,0.04)',
              },
            }}
          >
            <RetryIcon size={14} />
          </Box>
        )}
        <Box
          component="button"
          onClick={handleCopy}
          sx={{
            color: copied ? '#22c55e' : '#928F88',
            bgcolor: 'transparent',
            border: 'none',
            cursor: 'pointer',
            padding: '6px',
            borderRadius: '4px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            '&:hover': {
              color: copied ? '#22c55e' : colorInk,
              bgcolor: 'rgba(0,0,0,0.04)',
            },
          }}
        >
          {copied ? <Check size={14}/> : <CopyIcon size={14} />}
        </Box>
      </Box>
      <Box
        sx={{
          bgcolor: colorInk,
          color: '#F8F6F0',
          px: '12px',
          py: '8px',
          borderRadius: '10px 10px 2px 10px',
          fontFamily: fontFamilyBody,
          fontSize: '15.4px',
          fontWeight: 500,
          lineHeight: 1.5,
          maxWidth: '95%',
          wordBreak: 'break-word',
        }}
      >
        {content}
      </Box>
    </Box>
  );
}
