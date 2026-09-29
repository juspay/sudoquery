import { Box, Container, Paper } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import rehypeRaw from 'rehype-raw';
import { colorInk, fontFamilyDisplay, fontFamilyMono } from '../../theme/tokens';

const content = `
## Configuration Options

All configuration is done through the \`init()\` method:

<table>
<thead>
<tr><th>Option</th><th>Type</th><th>Default</th><th>Description</th></tr>
</thead>
<tbody>
<tr><td><code>flushInterval</code></td><td><code>number | undefined</code></td><td><code>undefined</code></td><td>Interval in milliseconds for periodic auto-flush. If not set, periodic flush is disabled.</td></tr>
<tr><td><code>batchSize</code></td><td><code>number | undefined</code></td><td><code>10</code></td><td>Number of events to accumulate before auto-flushing.</td></tr>
<tr><td><code>endpoint</code></td><td><code>string | undefined</code></td><td><code>"http://localhost:3000/push_batch"</code></td><td>URL where events are sent.</td></tr>
<tr><td><code>token</code></td><td><code>string | undefined</code></td><td><code>undefined</code></td><td>Project token for authentication. Required for sending events.</td></tr>
</tbody>
</table>

### Example Configurations

**Default configuration:**
\`\`\`typescript
HyperAnalytics.init();
// Uses: batchSize=10, endpoint="http://localhost:3000/push_batch", no periodic flush
\`\`\`

**High-frequency tracking:**
\`\`\`typescript
HyperAnalytics.init({
  flushInterval: 2000,   // Flush every 2 seconds
  batchSize: 50,         // Larger batches
  endpoint: 'https://analytics-api.example.com/batch'
});
\`\`\`

**Low-latency mode:**
\`\`\`typescript
HyperAnalytics.init({
  flushInterval: 1000,   // Flush every second
  batchSize: 5            // Small batches
});
\`\`\`
`;

export default function ConfigurationPage() {
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
            '& table': { width: '100%', borderCollapse: 'collapse', mb: 3 },
            '& th': { textAlign: 'left', p: 1.5, borderBottom: '2px solid', borderColor: 'divider', fontWeight: 600 },
            '& td': { textAlign: 'left', p: 1.5, borderBottom: '1px solid', borderColor: 'divider' },
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
