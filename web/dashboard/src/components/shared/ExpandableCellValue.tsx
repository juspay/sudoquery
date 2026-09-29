import { useMemo, useState } from 'react';
import {
  Box,
  Dialog,
  DialogContent,
  DialogTitle,
  IconButton,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import {
  Maximize2 as ExpandIcon,
  X as CloseIcon,
  Copy as CopyIcon,
  Check as CheckIcon,
} from 'lucide-react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark, oneLight } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { safeStringify } from '../../utils/cellValue';

const PREVIEW_MAX_LENGTH = 60;

interface ExpandableCellValueProps {
  value: unknown;
}

export function ExpandableCellValue({ value }: ExpandableCellValueProps) {
  const theme = useTheme();
  const isDark = theme.palette.mode === 'dark';
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const preview = useMemo(() => {
    const compact = safeStringify(value);
    return compact.length > PREVIEW_MAX_LENGTH
      ? `${compact.slice(0, PREVIEW_MAX_LENGTH)}…`
      : compact;
  }, [value]);

  const pretty = useMemo(() => safeStringify(value, 2), [value]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(pretty);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard unavailable (e.g. non-secure context) — nothing sensible to do
    }
  };

  return (
    <>
      <Box
        component="span"
        onClick={(e: React.MouseEvent) => {
          e.stopPropagation();
          setOpen(true);
        }}
        title="Click to expand"
        sx={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 0.5,
          maxWidth: '100%',
          px: 0.75,
          py: '1px',
          borderRadius: 1,
          bgcolor: 'action.hover',
          cursor: 'pointer',
          fontFamily: 'inherit',
          '&:hover': { bgcolor: 'action.selected' },
        }}
      >
        <Box
          component="span"
          sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
        >
          {preview}
        </Box>
        <ExpandIcon size={11} />
      </Box>

      <Dialog
        open={open}
        onClose={() => setOpen(false)}
        maxWidth="md"
        fullWidth
        onClick={(e) => e.stopPropagation()}
      >
        <DialogTitle
          sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
        >
          <Typography variant="subtitle1" fontWeight={600}>
            {Array.isArray(value) ? 'Array' : 'Object'}
          </Typography>
          <Box sx={{ display: 'flex', alignItems: 'center' }}>
            <Tooltip title={copied ? 'Copied' : 'Copy JSON'}>
              <IconButton size="small" onClick={handleCopy}>
                {copied ? <CheckIcon size={16} /> : <CopyIcon size={16} />}
              </IconButton>
            </Tooltip>
            <IconButton size="small" onClick={() => setOpen(false)}>
              <CloseIcon size={16} />
            </IconButton>
          </Box>
        </DialogTitle>
        <DialogContent sx={{ p: 0 }}>
          <SyntaxHighlighter
            language="json"
            style={isDark ? oneDark : oneLight}
            customStyle={{
              margin: 0,
              borderRadius: 0,
              fontSize: '0.8rem',
              maxHeight: '60vh',
            }}
            showLineNumbers
          >
            {pretty}
          </SyntaxHighlighter>
        </DialogContent>
      </Dialog>
    </>
  );
}
