import { Link as RouterLink } from 'react-router-dom';
import { Alert, Box, Button, CircularProgress, Skeleton, Typography } from '@mui/material';
import { ProfileTypeChip } from './Chips';
import { HAIRLINE, monoSx, overlineSx } from '../Discover/styles';
import { formatEpoch, formatMetric, formatTimestamp } from '../../cdp/format';
import type { CustomerRow } from '../../services/cdpService';
import { colorBlue, colorCream, colorInk60 } from '../../theme/tokens';

const GRID = 'minmax(200px, 2fr) minmax(140px, 1fr) 90px 110px 80px 120px 150px';

interface CustomerTableProps {
  rows: CustomerRow[];
  loading: boolean;
  error: string | null;
  nextCursor: string | null;
  loadingMore: boolean;
  onLoadMore: () => void;
  /** Label of the last column: when they joined, or entered the segment. */
  sinceLabel: string;
  empty: React.ReactNode;
}

/** Customers with their contacts and headline order metrics. */
export function CustomerTable({
  rows,
  loading,
  error,
  nextCursor,
  loadingMore,
  onLoadMore,
  sinceLabel,
  empty,
}: CustomerTableProps) {
  if (error) {
    return (
      <Alert severity="error" sx={{ m: 1.5 }}>
        {error}
      </Alert>
    );
  }
  if (loading && rows.length === 0) {
    return (
      <Box sx={{ p: 2 }}>
        {[0, 1, 2, 3, 4, 5].map((row) => (
          <Skeleton key={row} height={34} />
        ))}
      </Box>
    );
  }
  if (rows.length === 0) return <>{empty}</>;

  const headers = ['Email', 'Phone', 'Type', 'Total spend', 'Orders', 'Last order', sinceLabel];
  return (
    <Box sx={{ flex: 1, minHeight: 0, overflow: 'auto', opacity: loading ? 0.55 : 1 }}>
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: GRID,
          columnGap: 2,
          px: 2,
          py: 1,
          position: 'sticky',
          top: 0,
          zIndex: 1,
          bgcolor: '#FFFFFF',
          borderBottom: HAIRLINE,
        }}
      >
        {headers.map((label, i) => (
          <Box key={label} sx={{ ...overlineSx, textAlign: i === 3 || i === 4 ? 'right' : 'left' }}>
            {label}
          </Box>
        ))}
      </Box>

      {rows.map((row) => (
        <Box
          key={row.profile_id}
          component={RouterLink}
          to={`/app/customers/profiles/${encodeURIComponent(row.profile_id)}`}
          sx={{
            display: 'grid',
            gridTemplateColumns: GRID,
            columnGap: 2,
            alignItems: 'center',
            px: 2,
            py: 0.875,
            borderBottom: HAIRLINE,
            textDecoration: 'none',
            color: 'text.primary',
            '&:hover': { bgcolor: colorCream },
          }}
        >
          <Box
            sx={{ ...monoSx, color: colorBlue, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            title={row.email ?? row.profile_id}
          >
            {row.email ?? <Box component="span" sx={{ color: colorInk60 }}>{row.profile_id.slice(0, 8)}…</Box>}
          </Box>
          <Box sx={monoSx}>{row.phone ?? '—'}</Box>
          <Box>
            <ProfileTypeChip type={row.profile_type} />
          </Box>
          <Box sx={{ ...monoSx, textAlign: 'right' }}>{formatMetric(row.total_spend)}</Box>
          <Box sx={{ ...monoSx, textAlign: 'right' }}>{formatMetric(row.order_count)}</Box>
          <Box sx={monoSx}>{formatEpoch(row.last_order_at)}</Box>
          <Box sx={monoSx}>{formatTimestamp(row.entered_at ?? row.created_at)}</Box>
        </Box>
      ))}

      {nextCursor ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 1.5 }}>
          <Button size="small" variant="outlined" onClick={onLoadMore} disabled={loadingMore}>
            {loadingMore ? <CircularProgress size={14} sx={{ mr: 1 }} /> : null}
            Load more
          </Button>
        </Box>
      ) : (
        <Typography sx={{ fontSize: '12px', color: colorInk60, textAlign: 'center', py: 1.5 }}>
          All {rows.length.toLocaleString()} loaded
        </Typography>
      )}
    </Box>
  );
}
