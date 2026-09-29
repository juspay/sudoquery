import { Box, Container, Paper } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import rehypeRaw from 'rehype-raw';
import { colorInk, fontFamilyDisplay, fontFamilyMono } from '../../theme/tokens';

const content = `
## API Reference

### HyperAnalytics

All functionality is accessed through the \`HyperAnalytics\` class.

<table>
<thead>
<tr><th>Method</th><th>Description</th></tr>
</thead>
<tbody>
<tr><td><code>init(config?)</code></td><td>Initialize the SDK with optional configuration</td></tr>
<tr><td><code>setUser(userId: string)</code></td><td>Set current user ID</td></tr>
<tr><td><code>getUser(): string | null</code></td><td>Get current user ID</td></tr>
<tr><td><code>removeUser()</code></td><td>Clear user ID</td></tr>
<tr><td><code>track(eventName: string, properties: JSONSerializable)</code></td><td>Track an event</td></tr>
<tr><td><code>setSuperProperty(key: string, value: JSONSerializable)</code></td><td>Add a super property</td></tr>
<tr><td><code>getSuperProperties(): Record&lt;string, JSONSerializable&gt;</code></td><td>Get all super properties</td></tr>
<tr><td><code>clearSuperProperties()</code></td><td>Remove all super properties</td></tr>
<tr><td><code>flush(useBeacon?: boolean): Promise&lt;void&gt;</code></td><td>Manually flush pending events</td></tr>
<tr><td><code>get batchSize(): number</code></td><td>Get current batch size (read-only)</td></tr>
<tr><td><code>get endpoint(): string</code></td><td>Get current endpoint URL (read-only)</td></tr>
</tbody>
</table>

## Types

\`\`\`typescript
type JSONSerializable =
  | string
  | number
  | boolean
  | null
  | JSONSerializable[]
  | { [key: string]: JSONSerializable };
\`\`\`

## Browser Support

The SDK uses \`localStorage\` for persisting anonymous IDs and \`navigator.sendBeacon()\` for reliable delivery during page unload. Works in all modern browsers.

## License

ISC
`;

export default function ApiReferencePage() {
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
