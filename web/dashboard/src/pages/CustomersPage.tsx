import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Alert,
  Box,
  Button,
  MenuItem,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material';
import { CustomersHeader } from '../components/Customers/CustomersHeader';
import { CustomerTable } from '../components/Customers/CustomerTable';
import { panelSx } from '../components/Discover/styles';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { formatCount } from '../cdp/format';
import { usePaged } from '../cdp/usePaged';
import { useAsyncResult } from '../discover/useDiscover';
import { useProject } from '../contexts/ProjectContext';
import { apiErrorMessage } from '../services/apiClient';
import { cdpService, type ProfileType } from '../services/cdpService';
import { colorInk60 } from '../theme/tokens';

const LOOKUP_TYPES = ['email', 'phone', 'user_id', 'cookie', 'device_id'];

function Stat({ label, value }: { label: string; value: number | null | undefined }) {
  return (
    <Box sx={{ ...panelSx, px: 2, py: 1.25, minWidth: 140 }}>
      <Typography sx={{ fontSize: '11px', fontWeight: 600, color: colorInk60, textTransform: 'uppercase' }}>
        {label}
      </Typography>
      <Typography sx={{ fontSize: '22px', fontWeight: 700 }}>{formatCount(value)}</Typography>
    </Box>
  );
}

/** Find a profile by one of its identifiers. */
function Lookup() {
  const navigate = useNavigate();
  const [type, setType] = useState('email');
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const find = async () => {
    if (!value.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const profile = await cdpService.lookup(type, value.trim());
      navigate(`/app/customers/profiles/${encodeURIComponent(profile.profile_id)}`);
    } catch (e) {
      setError(apiErrorMessage(e, 'No profile has that identifier'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Box
      component="form"
      onSubmit={(e) => {
        e.preventDefault();
        void find();
      }}
      sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
    >
      <TextField select size="small" value={type} onChange={(e) => setType(e.target.value)} sx={{ width: 130 }}>
        {LOOKUP_TYPES.map((t) => (
          <MenuItem key={t} value={t}>
            {t}
          </MenuItem>
        ))}
      </TextField>
      <TextField
        size="small"
        placeholder="Find a customer…"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        error={error !== null}
        helperText={error ?? undefined}
        sx={{ width: 260 }}
      />
      <Button type="submit" variant="outlined" size="small" disabled={busy || !value.trim()}>
        Find
      </Button>
    </Box>
  );
}

export default function CustomersPage() {
  const { currentProject } = useProject();
  const [profileType, setProfileType] = useState<ProfileType | null>(null);
  const active = currentProject?.status === 'ACTIVE';
  const key = active ? `${currentProject.id}:${profileType ?? 'all'}` : null;

  const paged = usePaged(
    key,
    (cursor) => cdpService.customers(cursor, profileType),
    (p) => ({ items: p.profiles, next: p.next_cursor }),
  );
  // Totals are workspace-wide, so they don't depend on the filter.
  const totals = useAsyncResult(active ? currentProject.id : null, () => cdpService.totals()).data;

  if (!currentProject) {
    return (
      <EmptyStateCard heading="No project selected" subtext="Choose an organization and a project to see its customers." />
    );
  }
  if (!active) {
    return (
      <EmptyStateCard
        heading="This project is still being set up"
        subtext="Customers appear once the project is active and its events start arriving."
      />
    );
  }

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 1.5, p: 2, pt: 1 }}>
      <CustomersHeader tab="customers" />

      <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap', alignItems: 'center' }}>
        <Stat label="Profiles" value={totals?.workspace_members} />
        <Stat label="Customers" value={totals?.workspace_customers} />
        <Stat label="Visitors" value={totals?.workspace_visitors} />
        <Box sx={{ flex: 1 }} />
        <Lookup />
      </Box>

      <ToggleButtonGroup
        size="small"
        exclusive
        value={profileType ?? 'all'}
        onChange={(_, v: string | null) => {
          if (v) setProfileType(v === 'all' ? null : (v as ProfileType));
        }}
        sx={{ '& .MuiToggleButton-root': { py: 0.25, px: 1.5, fontSize: '12px', textTransform: 'none' } }}
      >
        <ToggleButton value="all">All</ToggleButton>
        <ToggleButton value="customer">Customers</ToggleButton>
        <ToggleButton value="visitor">Visitors</ToggleButton>
      </ToggleButtonGroup>

      <Box sx={{ ...panelSx, flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
        <CustomerTable
          rows={paged.rows}
          loading={paged.first.loading}
          error={paged.first.error}
          nextCursor={paged.nextCursor}
          loadingMore={paged.loadingMore}
          onLoadMore={() => void paged.loadMore()}
          sinceLabel="First seen"
          empty={
            <EmptyStateCard
              heading="No customers yet"
              subtext="Profiles are built from this project's events. Send events with identifiers (properties.identifiers, or an anon_id) and they appear here within seconds."
            />
          }
        />
        {paged.moreError ? (
          <Alert severity="error" sx={{ m: 1.5 }}>
            {paged.moreError}
          </Alert>
        ) : null}
      </Box>
    </Box>
  );
}
