import { Box, Container, Paper } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import rehypeRaw from 'rehype-raw';
import { colorInk, fontFamilyDisplay, fontFamilyMono } from '../../theme/tokens';

const content = `
## Features

- **Event Tracking** - Track custom events with properties
- **Automatic Batching** - Accumulate events locally to reduce network requests
- **Anonymous Users** - Track unauthenticated users with persistent anonymous IDs
- **Super Properties** - Attach default properties to all events
- **Auto-Flush** - Automatically sends events on page unload using \`navigator.sendBeacon()\`
- **Periodic Auto-Flush** - Configurable interval-based automatic event flushing
- **Cross-Platform** - Works in both browsers and Node.js
- **TypeScript Support** - Full type definitions included

## Installation

\`\`\`bash
npm install git+ssh://git@ssh.bitbucket.juspay.net/~sridatta.yalla_juspay.in/hyper-analytics-ts.git#main
\`\`\`

## Quick Start

\`\`\`typescript
import { HyperAnalytics } from 'hyper-analytics';

// Initialize the SDK
HyperAnalytics.init();

// Track an event
HyperAnalytics.track('button_click', {
  button_id: 'submit',
  page: '/checkout'
});
\`\`\`
`;

export default function GettingStartedPage() {
  return (
    <Box sx={{ p: { xs: 2, sm: 4, md: 6 }, maxWidth: 860, mx: 'auto' }}>
      <Paper elevation={0} sx={{ p: { xs: 3, md: 6 }, borderRadius: 2 }}>
        <Box
          sx={{
            '& h2': { fontSize: '1.5rem', fontFamily: fontFamilyDisplay, fontWeight: 600, color: colorInk, mt: 0, mb: 2 },
            '& h3': { fontSize: '1.25rem', fontWeight: 600, color: colorInk, mt: 3, mb: 1.5 },
            '& p': { color: 'text.primary', lineHeight: 1.7, mb: 2 },
            '& ul': { pl: 3, mb: 2 },
            '& li': { mb: 0.5, lineHeight: 1.6 },
            '& code': { fontFamily: fontFamilyMono, fontSize: '0.875rem', bgcolor: 'rgba(0,0,0,0.04)', px: 0.5, py: 0.25, borderRadius: 0.5 },
            '& pre': { margin: 0, borderRadius: 1 },
            '& strong': { fontWeight: 600, color: colorInk },
          }}
        >
          <ReactMarkdown
            rehypePlugins={[rehypeRaw]}
            components={{
              code({ node, inline, className, children, ...props }) {
                const match = /language-(\w+)/.exec(className || '');
                return !inline && match ? (
                  <SyntaxHighlighter
                    style={vscDarkPlus}
                    language={match[1]}
                    PreTag="div"
                    {...props}
                  >
                    {String(children).replace(/\n$/, '')}
                  </SyntaxHighlighter>
                ) : (
                  <code className={className} {...props}>
                    {children}
                  </code>
                );
              },
            }}
          >
            {content}
          </ReactMarkdown>
        </Box>
      </Paper>
    </Box>
  );
}
