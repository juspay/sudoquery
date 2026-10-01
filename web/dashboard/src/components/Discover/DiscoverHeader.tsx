import { Link as RouterLink, useLocation } from 'react-router-dom';
import { Box, CircularProgress, IconButton, Tab, Tabs, Tooltip } from '@mui/material';
import { RefreshCw as RefreshIcon } from 'lucide-react';
import { useAutoRefresh } from '../../discover/autoRefresh';
import { endsAtFixedTime } from '../../discover/timeRange';
import type { Discover } from '../../discover/useDiscover';
import type { FieldDef } from '../../discover/types';
import { colorCream2, colorInk60 } from '../../theme/tokens';
import { AutoRefreshPicker } from './AutoRefreshPicker';
import { FilterBar } from './FilterBar';
import { QueryBar } from './QueryBar';
import { TimeRangePicker } from './TimeRangePicker';

type DiscoverTab = 'events' | 'sessions';

const TAB_PATH: Record<DiscoverTab, string> = {
  events: '/app/discover',
  sessions: '/app/discover/sessions',
};

interface DiscoverHeaderProps {
  tab: DiscoverTab;
  discover: Discover;
  fields: FieldDef[];
  loading: boolean;
  lookupValues?: (field: string, prefix: string) => Promise<string[]>;
}

/** Query bar, time range and filter pills, shared by the events and sessions tabs. */
export function DiscoverHeader({ tab, discover, fields, loading, lookupValues }: DiscoverHeaderProps) {
  const location = useLocation();
  const { state, update, refresh, built } = discover;
  useAutoRefresh(state.refresh, refresh);

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
      <Tabs
        value={tab}
        sx={{
          minHeight: 34,
          borderBottom: `1px solid ${colorCream2}`,
          '& .MuiTab-root': { minHeight: 34, py: 0, fontSize: '13px', fontWeight: 600 },
        }}
      >
        {(Object.keys(TAB_PATH) as DiscoverTab[]).map((option) => (
          <Tab
            key={option}
            value={option}
            label={option === 'events' ? 'Events' : 'Sessions'}
            component={RouterLink}
            // The search carries over between tabs.
            to={{ pathname: TAB_PATH[option], search: location.search }}
          />
        ))}
      </Tabs>

      <Box sx={{ display: 'flex', alignItems: 'flex-start', gap: 1 }}>
        <QueryBar
          query={state.query}
          language={state.language}
          fields={fields}
          error={built.ok ? null : built.error}
          onSubmit={(query) => update({ query })}
          onLanguageChange={(language) => update({ language })}
          lookupValues={lookupValues}
        />
        <TimeRangePicker value={state.time} onChange={(time) => update({ time })} />
        <AutoRefreshPicker
          value={state.refresh}
          fixedEnd={endsAtFixedTime(state.time)}
          onChange={(next) => update({ refresh: next })}
        />
        <Tooltip title="Refresh">
          <IconButton
            onClick={refresh}
            aria-label="Refresh"
            sx={{ width: 40, height: 40, color: colorInk60 }}
          >
            {loading ? <CircularProgress size={16} /> : <RefreshIcon size={16} />}
          </IconButton>
        </Tooltip>
      </Box>

      <FilterBar
        filters={state.filters}
        fields={fields}
        onChange={(filters) => update({ filters })}
      />
    </Box>
  );
}
