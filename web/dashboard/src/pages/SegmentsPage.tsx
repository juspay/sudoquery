import { useState } from 'react';
import { Link as RouterLink, useNavigate } from 'react-router-dom';
import { Alert, Box, Button, Skeleton, Typography } from '@mui/material';
import { Plus as PlusIcon } from 'lucide-react';
import { StatusChip } from '../components/Customers/Chips';
import { CustomersHeader } from '../components/Customers/CustomersHeader';
import { SegmentBuilderDialog } from '../components/Customers/SegmentBuilderDialog';
import { HAIRLINE, monoSx, overlineSx, panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { describePredicate } from '../cdp/predicate';
import { formatCount, formatTimestamp } from '../cdp/format';
import { useAsyncResult } from '../discover/useDiscover';
import { useProject } from '../contexts/ProjectContext';
import { cdpService } from '../services/cdpService';
import { colorCream, colorInk60 } from '../theme/tokens';

const GRID = 'minmax(220px, 2fr) minmax(260px, 3fr) 100px 110px 150px';

export default function SegmentsPage() {
  const navigate = useNavigate();
  const { currentProject } = useProject();
  const [creating, setCreating] = useState(false);
  const [reload, setReload] = useState(0);
  const key = currentProject && currentProject.status === 'ACTIVE' ? `${currentProject.id}:${reload}` : null;
  const segments = useAsyncResult(key, () => cdpService.segments());

  if (!currentProject) {
    return (
      <EmptyStateCard heading="No project selected" subtext="Choose an organization and a project to see its segments." />
    );
  }

  const rows = segments.data ?? [];
  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <CustomersHeader tab="segments" />

      <Box sx={{ display: 'flex', alignItems: 'center' }}>
        <Typography sx={{ fontSize: '12.5px', color: colorInk60, flex: 1 }}>
          Segments are rules over customer metrics. Membership updates live as events arrive.
        </Typography>
        <Button variant="contained" size="small" startIcon={<PlusIcon size={14} />} onClick={() => setCreating(true)}>
          New segment
        </Button>
      </Box>

      <Box sx={{ ...panelSx, flex: 1, minHeight: 0, overflow: 'auto' }}>
        {segments.error ? (
          <Alert severity="error" sx={{ m: 1.5 }}>
            {segments.error}
          </Alert>
        ) : !segments.data && segments.loading ? (
          <Box sx={{ p: 2 }}>
            {[0, 1, 2, 3].map((row) => (
              <Skeleton key={row} height={34} />
            ))}
          </Box>
        ) : rows.length === 0 ? (
          <EmptyStateCard
            heading="No segments yet"
            subtext="Create one to group customers by what they do — e.g. spent over 5,000, or ordered in the last 30 days."
            ctaLabel="New segment"
            onCta={() => setCreating(true)}
          />
        ) : (
          <>
            <Box
              sx={{
                display: 'grid',
                gridTemplateColumns: GRID,
                columnGap: 2,
                px: 2,
                py: 1,
                position: 'sticky',
                top: 0,
                bgcolor: '#FFFFFF',
                borderBottom: HAIRLINE,
              }}
            >
              {['Segment', 'Rule', 'Status', 'Members', 'Created'].map((label, i) => (
                <Box key={label} sx={{ ...overlineSx, textAlign: i === 3 ? 'right' : 'left' }}>
                  {label}
                </Box>
              ))}
            </Box>
            {rows.map((s) => (
              <Box
                key={s.segment_id}
                component={RouterLink}
                to={`/app/customers/segments/${encodeURIComponent(s.segment_id)}`}
                sx={{
                  display: 'grid',
                  gridTemplateColumns: GRID,
                  columnGap: 2,
                  alignItems: 'center',
                  px: 2,
                  py: 1,
                  borderBottom: HAIRLINE,
                  textDecoration: 'none',
                  color: 'text.primary',
                  '&:hover': { bgcolor: colorCream },
                }}
              >
                <Box sx={{ minWidth: 0 }}>
                  <Typography sx={{ fontSize: '13.5px', fontWeight: 600 }} noWrap>
                    {s.display_name}
                  </Typography>
                  <Typography sx={{ ...monoSx, color: colorInk60 }} noWrap>
                    {s.segment_id}
                    {s.is_temporal ? ' · time-based' : ''}
                  </Typography>
                </Box>
                <Typography sx={{ fontSize: '12.5px', color: colorInk60 }} noWrap title={describePredicate(s.predicate)}>
                  {describePredicate(s.predicate)}
                </Typography>
                <Box>
                  <StatusChip status={s.status} />
                </Box>
                <Box sx={{ ...monoSx, textAlign: 'right' }}>{formatCount(s.member_count)}</Box>
                <Box sx={monoSx}>{formatTimestamp(s.created_at)}</Box>
              </Box>
            ))}
          </>
        )}
      </Box>

      {/* Mounted only while open, so every new segment starts from a blank form. */}
      {creating ? (
        <SegmentBuilderDialog
          open
          onClose={() => setCreating(false)}
          onCreated={(id) => {
            setCreating(false);
            setReload((n) => n + 1);
            navigate(`/app/customers/segments/${encodeURIComponent(id)}`);
          }}
        />
      ) : null}
    </Box>
  );
}
