import { Box, Container, Paper } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import rehypeRaw from 'rehype-raw';
import { colorInk, fontFamilyDisplay, fontFamilyMono } from '../../theme/tokens';

const content = `
### Initialization

Call \`init()\` once when your application loads. You can optionally configure:

\`\`\`typescript
import { HyperAnalytics } from 'hyper-analytics';

HyperAnalytics.init({
  flushInterval: 5000,    // Auto-flush every 5 seconds (optional)
  batchSize: 20,          // Batch 20 events before flushing (default: 10)
  endpoint: 'https://api.example.com/events',  // Custom endpoint (default: http://localhost:3000/push_batch)
  token: 'YOUR_PROJECT_TOKEN'  // Project token for authentication (required)
});
\`\`\`

**Important:** Configuration can only be set during initialization and cannot be modified afterward.

This sets up:
- A page visibility listener to automatically flush events when the user navigates away
- A periodic flush timer (if \`flushInterval\` is provided)

### Identifying Users

Set a user ID to associate events with authenticated users:

\`\`\`typescript
// Set user ID
HyperAnalytics.setUser('user_123');

// Get current user ID
const userId = HyperAnalytics.getUser();

// Clear user ID (on logout)
HyperAnalytics.removeUser();
\`\`\`

### Tracking Events

Track events with custom properties. Properties must be JSON serializable (primitives only - no nested objects):

\`\`\`typescript
HyperAnalytics.track('page_view', {
  page: '/home',
  referrer: 'https://google.com'
});

HyperAnalytics.track('purchase', {
  product_id: 'prod_456',
  price: 29.99,
  quantity: 2
});
\`\`\`

### Super Properties

Attach default properties to every event:

\`\`\`typescript
// Add super properties
HyperAnalytics.setSuperProperty('app_version', '1.0.0');
HyperAnalytics.setSuperProperty('environment', 'production');

// Get all super properties
const props = HyperAnalytics.getSuperProperties();

// Clear all super properties
HyperAnalytics.clearSuperProperties();
\`\`\`

### Manual Flush

Force upload of pending events:

\`\`\`typescript
await HyperAnalytics.flush();
\`\`\`
`;

export default function UsagePage() {
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
