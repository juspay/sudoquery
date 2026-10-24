import { useState, useEffect, useMemo } from 'react';
import { Box, Typography, TextField, Button, Alert, IconButton, Tooltip, Paper, Divider, Collapse, Link, Autocomplete } from '@mui/material';
import { Copy, Eye, EyeOff, ChevronDown, ChevronUp, ExternalLink, Loader2 } from 'lucide-react';
import { Link as RouterLink } from 'react-router-dom';
import countriesAndTimezones from 'countries-and-timezones';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useToast } from '../../contexts/ToastContext';
import { workspaceService } from '../../services/workspaceService';
import { CopyableId } from '../shared/CopyableId';
import { colorInk, colorCream2, colorBluePale, fontFamilyMono } from '../../theme/tokens';

interface TimezoneOption {
  value: string;
  label: string;
  country: string;
}

// Build timezone options from countries-and-timezones
function buildTimezoneOptions(): TimezoneOption[] {
  const allTimezones = countriesAndTimezones.getAllTimezones();
  const allCountries = countriesAndTimezones.getAllCountries();

  const options: TimezoneOption[] = [];

  for (const [tzName, tz] of Object.entries(allTimezones)) {
    const countryName = tz.countries?.[0] ? allCountries[tz.countries[0]]?.name || '' : '';
    const offset = tz.utcOffsetStr || '';
    const label = `${tzName} (${countryName}, UTC${offset})`;
    options.push({
      value: tzName,
      label,
      country: countryName,
    });
  }

  return options.sort((a, b) => a.value.localeCompare(b.value));
}

const TIMEZONE_OPTIONS = buildTimezoneOptions();
const DEFAULT_TIMEZONE = 'Asia/Kolkata';

export function WorkspaceSettingsForm() {
  const { currentWorkspace, setCurrentWorkspace, refreshWorkspaces } = useWorkspace();
  const { currentOrganization } = useOrganization();
  const { showSuccess, showError } = useToast();

  const [name, setName] = useState(currentWorkspace?.name || '');
  const [timezone, setTimezone] = useState<string>(currentWorkspace?.timezone || DEFAULT_TIMEZONE);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showToken, setShowToken] = useState(true);
  const [workspaceToken, setWorkspaceToken] = useState<string>('');
  const [loadingToken, setLoadingToken] = useState(false);
  const [tokenExpanded, setTokenExpanded] = useState(false);
  const [timezoneSearch, setTimezoneSearch] = useState('');

  const selectedTimezoneOption = useMemo(() => {
    return TIMEZONE_OPTIONS.find(opt => opt.value === timezone) || TIMEZONE_OPTIONS.find(opt => opt.value === DEFAULT_TIMEZONE) || null;
  }, [timezone]);

  const filteredTimezoneOptions = useMemo(() => {
    if (!timezoneSearch.trim()) return TIMEZONE_OPTIONS;
    const search = timezoneSearch.toLowerCase();
    return TIMEZONE_OPTIONS.filter(opt =>
      opt.value.toLowerCase().includes(search) ||
      opt.country.toLowerCase().includes(search)
    );
  }, [timezoneSearch]);

  // Fetch workspace token on demand
  useEffect(() => {
    if (!currentOrganization?.id || !currentWorkspace?.id) return;

    const fetchToken = async () => {
      setLoadingToken(true);
      try {
        const response = await workspaceService.getWorkspaceTokens(currentOrganization.id, currentWorkspace.id);
        // Response is an object with numeric keys, e.g. { "0": { token, name, ... } }
        const tokens = Object.values(response);
        if (tokens.length > 0) {
          setWorkspaceToken(tokens[0].token);
        }
      } catch (err) {
        showError(err instanceof Error ? err.message : 'Failed to fetch workspace token');
      } finally {
        setLoadingToken(false);
      }
    };

    fetchToken();
  }, [currentOrganization?.id, currentWorkspace?.id, showError]);

  if (!currentWorkspace) return null;

  // Sync timezone with current workspace
  useEffect(() => {
    if (currentWorkspace?.timezone) {
      setTimezone(currentWorkspace.timezone);
    }
  }, [currentWorkspace?.timezone]);

  if (!currentWorkspace) return null;

  const handleSave = async () => {
    if (!currentOrganization?.id) return;
    if (!name.trim()) return;

    const updates: { name?: string; timezone?: string } = {};
    if (name.trim() !== currentWorkspace.name) {
      updates.name = name.trim();
    }
    if (timezone !== currentWorkspace.timezone) {
      updates.timezone = timezone;
    }

    if (Object.keys(updates).length === 0) return;

    setLoading(true);
    setError(null);

    try {
      const updated = await workspaceService.update(currentOrganization.id, currentWorkspace.id, updates);
      setCurrentWorkspace(updated);
      await refreshWorkspaces();
      showSuccess('Workspace updated successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update workspace');
    } finally {
      setLoading(false);
    }
  };

  const handleCopyToken = () => {
    if (workspaceToken) {
      navigator.clipboard.writeText(workspaceToken);
      showSuccess('Workspace token copied to clipboard');
    }
  };

  const isDirty = name.trim() !== currentWorkspace.name || timezone !== currentWorkspace.timezone;

  return (
    <Box sx={{ p: { xs: 3, sm: 4 }, border: `1px solid ${colorCream2}`, borderRadius: 2, bgcolor: '#fff' }}>
      <Typography variant="h6" color={colorInk} mb={3}>
        Workspace Details
      </Typography>

      {error && <Alert severity="error" sx={{ mb: 3 }}>{error}</Alert>}

      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3, maxWidth: 520 }}>
        <TextField
          label="Workspace Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={loading}
          fullWidth
        />

        <CopyableId
          label="Workspace ID"
          value={currentWorkspace.id}
          helperText="Server-generated slug that identifies your workspace in API calls and the SDK. It cannot be changed."
        />

        <Autocomplete
          value={selectedTimezoneOption}
          onChange={(_, newValue) => {
            if (newValue) {
              setTimezone(newValue.value);
            }
          }}
          inputValue={timezoneSearch}
          onInputChange={(_, newInputValue) => {
            setTimezoneSearch(newInputValue);
          }}
          options={filteredTimezoneOptions}
          getOptionLabel={(option) => option.label}
          isOptionEqualToValue={(option, value) => option.value === value.value}
          renderInput={(params) => (
            <TextField
              {...params}
              label="Timezone"
              placeholder="Search by country or timezone"
              helperText="Used by AI to infer the timezone of your data"
              fullWidth
            />
          )}
          renderOption={(props, option) => {
            const { key, ...otherProps } = props as any;
            return (
              <li key={key} {...otherProps}>
                <Box>
                  <Typography variant="body2">{option.value}</Typography>
                  <Typography variant="caption" color="text.secondary">
                    {option.country}
                  </Typography>
                </Box>
              </li>
            );
          }}
          disableClearable
        />

        <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 2 }}>
          <Button
            variant="contained"
            onClick={handleSave}
            disabled={!isDirty || loading || !name.trim()}
          >
            {loading ? 'Saving…' : 'Save Changes'}
          </Button>
        </Box>
      </Box>

      <Divider sx={{ my: 4 }} />

      {/* Workspace Token Section - Collapsible */}
      <Box
        onClick={() => setTokenExpanded(!tokenExpanded)}
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          cursor: 'pointer',
          mb: tokenExpanded ? 2 : 0,
        }}
      >
        <Box>
          <Typography variant="h6" color={colorInk}>
            API Token
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Use this token to push events from your backend or SDK.
          </Typography>
        </Box>
        <IconButton size="small" sx={{ color: 'text.secondary' }}>
          {tokenExpanded ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
        </IconButton>
      </Box>

      <Collapse in={tokenExpanded}>
      <Paper elevation={0} sx={{ bgcolor: colorBluePale, p: 3, borderRadius: 2, mb: 3 }}>
        {currentOrganization?.id ? (
          <Box sx={{ mb: 2 }}>
            <CopyableId
              label="Organization ID"
              value={currentOrganization.id}
              helperText="Sent as the X-Organization-Id header in API requests."
            />
          </Box>
        ) : null}
        <Box sx={{ mb: 2 }}>
          <CopyableId
            label="Workspace ID"
            value={currentWorkspace.id}
            helperText="Sent as the X-Workspace-Id header in API requests."
          />
        </Box>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', mb: 2 }}>
          <Typography
            variant="body2"
            sx={{
              fontFamily: fontFamilyMono,
              fontSize: '0.875rem',
              wordBreak: 'break-all',
              flex: 1,
              color: workspaceToken ? 'text.primary' : 'text.secondary',
            }}
          >
            {loadingToken ? 'Loading...' : (workspaceToken ? (showToken ? workspaceToken : '•'.repeat(workspaceToken.length)) : 'No token generated yet')}
          </Typography>
          {loadingToken && <Loader2 size={16} className="animate-spin" />}
          <Tooltip title={showToken ? 'Hide Token' : 'Show Token'}>
            <IconButton
              onClick={() => setShowToken(!showToken)}
              disabled={!workspaceToken || loadingToken}
              size="small"
              sx={{ color: 'text.secondary' }}
            >
              {showToken ? <EyeOff size={16} /> : <Eye size={16} />}
            </IconButton>
          </Tooltip>
          <Tooltip title="Copy Token">
            <IconButton
              onClick={handleCopyToken}
              disabled={!workspaceToken || loadingToken}
              size="small"
              sx={{ color: 'text.secondary' }}
            >
              <Copy size={16} />
            </IconButton>
          </Tooltip>
        </Box>

        <Typography variant="caption" color="text.secondary">
          <Link component={RouterLink} to="/docs" sx={{ fontWeight: 500, display: 'inline-flex', alignItems: 'center', gap: 0.5 }}>
            Learn how to integrate SDK
            <ExternalLink size={12} />
          </Link>
        </Typography>
      </Paper>
      </Collapse>
    </Box>
  );
}
