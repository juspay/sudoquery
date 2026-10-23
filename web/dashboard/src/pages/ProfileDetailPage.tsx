import { Link as RouterLink, useParams } from 'react-router-dom';
import { Alert, Box, Button, Link, Skeleton, Typography } from '@mui/material';
import { ArrowLeft as BackIcon } from 'lucide-react';
import { ProfileTypeChip } from '../components/Customers/Chips';
import { HAIRLINE, monoSx, overlineSx, panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { formatMetricValue, formatTimestamp } from '../cdp/format';
import { useAsyncResult } from '../discover/useDiscover';
import { useProject } from '../contexts/ProjectContext';
import { cdpService } from '../services/cdpService';
import { colorBlue, colorInk60 } from '../theme/tokens';

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Box sx={{ ...panelSx, display: 'flex', flexDirection: 'column' }}>
      <Box sx={{ ...overlineSx, px: 2, py: 1, borderBottom: HAIRLINE }}>{title}</Box>
      {children}
    </Box>
  );
}

function Row({ cells, grid }: { cells: React.ReactNode[]; grid: string }) {
  return (
    <Box
      sx={{
        display: 'grid',
        gridTemplateColumns: grid,
        columnGap: 2,
        alignItems: 'center',
        px: 2,
        py: 0.75,
        borderBottom: HAIRLINE,
        '&:last-child': { borderBottom: 'none' },
      }}
    >
      {cells}
    </Box>
  );
}

export default function ProfileDetailPage() {
  const { profileId = '' } = useParams();
  const { currentProject } = useProject();
  const key = currentProject && currentProject.status === 'ACTIVE' ? `${currentProject.id}:${profileId}` : null;
  const profile = useAsyncResult(key, () => cdpService.profile(profileId));

  if (!currentProject) {
    return <EmptyStateCard heading="No project selected" subtext="Choose an organization and a project." />;
  }

  const p = profile.data;
  const email = p?.identifiers.find((i) => i.type === 'email');
  const title = email?.value ?? email?.masked_value ?? (p ? `Profile ${p.profile_id.slice(0, 8)}` : profileId);

  return (
    <Box sx={{ height: '100%', overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <Box>
        <Button component={RouterLink} to="/app/customers" size="small" startIcon={<BackIcon size={14} />} sx={{ fontSize: '12px', color: colorInk60 }}>
          Customers
        </Button>
      </Box>

      {profile.error ? <Alert severity="error">{profile.error}</Alert> : null}
      {!p && profile.loading ? <Skeleton height={160} /> : null}

      {p ? (
        <>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Typography sx={{ fontSize: '20px', fontWeight: 700 }}>{title}</Typography>
            <ProfileTypeChip type={p.profile_type} />
          </Box>
          <Typography sx={{ ...monoSx, color: colorInk60 }}>
            {p.profile_id} · first seen {formatTimestamp(p.created_at)}
          </Typography>

          <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', lg: '1fr 1fr' }, gap: 1.5 }}>
            <Section title="Identifiers">
              {p.identifiers.map((i) => (
                <Row
                  key={`${i.type}:${i.masked_value ?? i.value}`}
                  grid="110px 1fr 90px"
                  cells={[
                    <Box key="t" sx={{ fontSize: '13px', fontWeight: 600 }}>
                      {i.type}
                    </Box>,
                    <Box key="v" sx={monoSx}>
                      {i.value ?? i.masked_value ?? '—'}
                    </Box>,
                    <Box key="tier" sx={{ fontSize: '12px', color: colorInk60 }}>
                      {i.tier}
                    </Box>,
                  ]}
                />
              ))}
            </Section>

            <Section title="Metrics">
              {p.metrics.length === 0 ? (
                <Typography sx={{ px: 2, py: 1.5, fontSize: '13px', color: colorInk60 }}>No metrics yet.</Typography>
              ) : (
                p.metrics.map((m) => (
                  <Row
                    key={m.metric}
                    grid="1fr 160px"
                    cells={[
                      <Box key="n" sx={{ fontSize: '13px' }}>
                        {m.display_name ?? m.metric}
                      </Box>,
                      <Box key="v" sx={{ ...monoSx, textAlign: 'right' }}>
                        {formatMetricValue(m.metric, m.value)}
                      </Box>,
                    ]}
                  />
                ))
              )}
            </Section>
          </Box>

          <Section title="Segments">
            {p.segments.length === 0 ? (
              <Typography sx={{ px: 2, py: 1.5, fontSize: '13px', color: colorInk60 }}>
                Not in any segment.
              </Typography>
            ) : (
              p.segments.map((s) => (
                <Row
                  key={s.segment_id}
                  grid="1fr 90px 170px 170px"
                  cells={[
                    <Link
                      key="n"
                      component={RouterLink}
                      to={`/app/customers/segments/${encodeURIComponent(s.segment_id)}`}
                      underline="hover"
                      sx={{ fontSize: '13px', color: colorBlue }}
                    >
                      {s.display_name ?? s.segment_id}
                    </Link>,
                    <Box key="s" sx={{ fontSize: '12px', textTransform: 'capitalize' }}>
                      {s.state}
                    </Box>,
                    <Box key="e" sx={monoSx}>
                      {formatTimestamp(s.entered_at)}
                    </Box>,
                    <Box key="x" sx={monoSx}>
                      {s.exited_at ? `left ${formatTimestamp(s.exited_at)}` : ''}
                    </Box>,
                  ]}
                />
              ))
            )}
          </Section>
        </>
      ) : null}
    </Box>
  );
}
