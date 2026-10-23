import { useState, useEffect, useMemo } from 'react';
import { Box, Typography, TextField, Button, Alert, Autocomplete } from '@mui/material';
import countriesAndTimezones from 'countries-and-timezones';
import { useProject } from '../../contexts/ProjectContext';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useToast } from '../../contexts/ToastContext';
import { projectService } from '../../services/projectService';
import { CopyableId } from '../shared/CopyableId';
import { colorInk, colorCream2 } from '../../theme/tokens';

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

export function ProjectSettingsForm() {
  const { currentProject, setCurrentProject, refreshProjects } = useProject();
  const { currentOrganization } = useOrganization();
  const { showSuccess } = useToast();

  const [name, setName] = useState(currentProject?.name || '');
  const [timezone, setTimezone] = useState<string>(currentProject?.timezone || DEFAULT_TIMEZONE);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
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

  // Sync timezone with current project
  useEffect(() => {
    if (currentProject?.timezone) {
      setTimezone(currentProject.timezone);
    }
  }, [currentProject?.timezone]);

  if (!currentProject) return null;

  const handleSave = async () => {
    if (!currentOrganization?.id) return;
    if (!name.trim()) return;

    const updates: { name?: string; timezone?: string } = {};
    if (name.trim() !== currentProject.name) {
      updates.name = name.trim();
    }
    if (timezone !== currentProject.timezone) {
      updates.timezone = timezone;
    }

    if (Object.keys(updates).length === 0) return;

    setLoading(true);
    setError(null);

    try {
      const updated = await projectService.update(currentOrganization.id, currentProject.id, updates);
      setCurrentProject(updated);
      await refreshProjects();
      showSuccess('Project updated successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update project');
    } finally {
      setLoading(false);
    }
  };

  const isDirty = name.trim() !== currentProject.name || timezone !== currentProject.timezone;

  return (
    <Box sx={{ p: { xs: 3, sm: 4 }, border: `1px solid ${colorCream2}`, borderRadius: 2, bgcolor: '#fff' }}>
      <Typography variant="h6" color={colorInk} mb={3}>
        Project Details
      </Typography>

      {error && <Alert severity="error" sx={{ mb: 3 }}>{error}</Alert>}

      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3, maxWidth: 520 }}>
        <TextField
          label="Project Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={loading}
          fullWidth
        />

        <CopyableId
          label="Project ID"
          value={currentProject.id}
          helperText="Server-generated slug that identifies your project in API calls and the SDK. It cannot be changed."
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
    </Box>
  );
}
