import { useState } from 'react';
import { Link as RouterLink, useParams } from 'react-router-dom';
import { Alert, Box, Button, Skeleton, Typography } from '@mui/material';
import { ArrowLeft as BackIcon } from 'lucide-react';
import { StatusChip } from '../components/Customers/Chips';
import { CustomerTable } from '../components/Customers/CustomerTable';
import { monoSx, panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { describePredicate } from '../cdp/predicate';
import { formatCount } from '../cdp/format';
import { usePaged } from '../cdp/usePaged';
import { useAsyncResult } from '../discover/useDiscover';
import { useProject } from '../contexts/ProjectContext';
import { apiErrorMessage } from '../services/apiClient';
import { cdpService, type SegmentStatus } from '../services/cdpService';
import { colorInk60 } from '../theme/tokens';

/** The status changes offered from each status; segments are otherwise immutable. */
const NEXT_STATUS: Record<SegmentStatus, Array<{ to: SegmentStatus; label: string }>> = {
  draft: [{ to: 'active', label: 'Activate' }],
  active: [
    { to: 'paused', label: 'Pause' },
    { to: 'archived', label: 'Archive' },
  ],
  paused: [
    { to: 'active', label: 'Resume' },
    { to: 'archived', label: 'Archive' },
  ],
  archived: [{ to: 'active', label: 'Reactivate' }],
};

export default function SegmentDetailPage() {
  const { segmentId = '' } = useParams();
  const { currentProject } = useProject();
  const [reload, setReload] = useState(0);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [changing, setChanging] = useState(false);
  const base = currentProject && currentProject.status === 'ACTIVE' ? `${currentProject.id}:${segmentId}` : null;

  const segment = useAsyncResult(base && `${base}:${reload}`, () => cdpService.segment(segmentId));
  const members = usePaged(
    base,
    (cursor) => cdpService.members(segmentId, cursor),
    (p) => ({ items: p.members, next: p.next_cursor }),
  );

  if (!currentProject) {
    return <EmptyStateCard heading="No project selected" subtext="Choose an organization and a project." />;
  }

  const setStatus = async (status: SegmentStatus) => {
    setChanging(true);
    setStatusError(null);
    try {
      await cdpService.setSegmentStatus(segmentId, status);
      setReload((n) => n + 1);
    } catch (e) {
      setStatusError(apiErrorMessage(e, 'Could not change the status'));
    } finally {
      setChanging(false);
    }
  };

  const s = segment.data;
  const header = members.first.data;
  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <Box>
        <Button component={RouterLink} to="/app/customers/segments" size="small" startIcon={<BackIcon size={14} />} sx={{ fontSize: '12px', color: colorInk60 }}>
          Segments
        </Button>
      </Box>

      {segment.error ? <Alert severity="error">{segment.error}</Alert> : null}
      {!s && segment.loading ? <Skeleton height={80} /> : null}

      {s ? (
        <Box sx={{ ...panelSx, p: 2, display: 'flex', flexDirection: 'column', gap: 1 }}>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Typography sx={{ fontSize: '18px', fontWeight: 700 }}>{s.display_name}</Typography>
            <StatusChip status={s.status} />
            <Box sx={{ flex: 1 }} />
            {NEXT_STATUS[s.status].map(({ to, label }) => (
              <Button key={to} size="small" variant="outlined" disabled={changing} onClick={() => void setStatus(to)}>
                {label}
              </Button>
            ))}
          </Box>
          <Typography sx={{ ...monoSx, color: colorInk60 }}>
            {s.segment_id}
            {s.is_temporal ? ' · time-based: membership is re-checked as days pass' : ''}
          </Typography>
          <Typography sx={{ fontSize: '13.5px' }}>
            Profiles where <strong>{describePredicate(s.predicate)}</strong>
          </Typography>
          {header ? (
            <Typography sx={{ fontSize: '12.5px', color: colorInk60 }}>
              {formatCount(header.total_members)} in this segment ({formatCount(header.segment_customers)} customers,{' '}
              {formatCount(header.segment_visitors)} visitors) of {formatCount(header.workspace_members)} profiles
            </Typography>
          ) : null}
          {statusError ? <Alert severity="error">{statusError}</Alert> : null}
        </Box>
      ) : null}

      <Box sx={{ ...panelSx, flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
        <CustomerTable
          rows={members.rows}
          loading={members.first.loading}
          error={members.first.error}
          nextCursor={members.nextCursor}
          loadingMore={members.loadingMore}
          onLoadMore={() => void members.loadMore()}
          sinceLabel="Entered"
          empty={
            <EmptyStateCard
              heading="Nobody in this segment yet"
              subtext="Profiles enter as soon as their metrics match the rule; a new segment backfills existing profiles in the background."
            />
          }
        />
        {members.moreError ? (
          <Alert severity="error" sx={{ m: 1.5 }}>
            {members.moreError}
          </Alert>
        ) : null}
      </Box>
    </Box>
  );
}
