import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Box,
  Button,
  Grid,
  Paper,
  Typography,
  TextField,
  IconButton,
  Menu,
  MenuItem,
  Fab,
  Backdrop,

} from '@mui/material';
import { Plus as AddIcon, MoreVertical as MoreVertIcon, LayoutDashboard as DashboardIcon, X as CloseIcon, RefreshCw as RefreshIcon } from 'lucide-react';
import { EmptyStateCard } from '../components/shared/EmptyStateCard';
import { InlineMetric, type ActionItem } from '../components/Chat/InlineMetric';
import { dashboardService, type Dashboard } from '../services/dashboardService';
import { useToast } from '../contexts/ToastContext';
import { useProject } from '../contexts/ProjectContext';
import { useOrganization } from '../contexts/OrganizationContext';
import { trackEvent, trackDashboardCreationOpened, trackDashboardCreationClosed} from '../utils/analytics';
import { PageHeader } from '../components/shared/PageHeader';

export default function DashboardPage() {
  const navigate = useNavigate();
  const { currentOrganization } = useOrganization();
  const { currentProject } = useProject();
  const { showSuccess, showError } = useToast();
  const [dashboards, setDashboards] = useState<Dashboard[]>([]);
  const [loading, setLoading] = useState(true);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [description, setDescription] = useState('');
  const [creating, setCreating] = useState(false);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  useEffect(() => {
    fetchDashboards();
  }, [currentOrganization, currentProject]);

  useEffect(() => {
    trackEvent('page_view', { page: 'dashboard' });
  }, []);

  // Track dashboard creation dialog open/close
  useEffect(() => {
    if (createDialogOpen) {
      trackDashboardCreationOpened();
    }
  }, [createDialogOpen]);

  const handleCloseCreateDialog = () => {
    trackDashboardCreationClosed();
    setCreateDialogOpen(false);
  };

  const fetchDashboards = async () => {
    if (!currentOrganization || !currentProject) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const response = await dashboardService.list(currentOrganization.id, currentProject.id);
      setDashboards(response);

      // Fetch fresh data for each dashboard, preserving order by index
      const updates = await Promise.all(
        response.map(async (dashboard) => {
          try {
            return await dashboardService.get(currentOrganization.id, currentProject.id, dashboard.id);
          } catch (err) {
            console.error(`Failed to refresh dashboard ${dashboard.id}:`, err);
            return dashboard; // Return cached data if refresh fails
          }
        })
      );

      // Updates array is in same order as response array
      setDashboards(updates);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load dashboards';
      showError(message);
    } finally {
      setLoading(false);
    }
  };

  const handleCreate = async () => {
    if (!description.trim() || !currentOrganization || !currentProject) return;

    setCreating(true);
    try {
      await dashboardService.create(currentOrganization.id, currentProject.id, {
        description: description.trim(),
        query: '', // Placeholder for now
      });
      showSuccess('Dashboard created successfully.');
      trackEvent('dashboard_created', { project_id: currentProject.id });
      setCreateDialogOpen(false);
      setDescription('');
      fetchDashboards();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to create dashboard';
      showError(message);
    } finally {
      setCreating(false);
    }
  };

  const handleRefresh = async (e: React.MouseEvent, dashboard: Dashboard) => {
    e.stopPropagation();
    await handleRefreshStandalone(dashboard);
  };

  const handleRefreshStandalone = async (dashboard: Dashboard) => {
    if (!currentOrganization || !currentProject) return;

    setRefreshingId(dashboard.id);
    try {
      const updated = await dashboardService.get(currentOrganization.id, currentProject.id, dashboard.id);
      setDashboards((prev) => prev.map((d) => (d.id === updated.id ? updated : d)));
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to refresh dashboard';
      showError(message);
    } finally {
      setRefreshingId(null);
    }
  };

  const handleDeleteStandalone = async (dashboard: Dashboard) => {
    if (!currentOrganization || !currentProject) return;

    try {
      await dashboardService.delete(currentOrganization.id, currentProject.id, dashboard.id);
      setDashboards((prev) => prev.filter((d) => d.id !== dashboard.id));
      showSuccess('Dashboard deleted.');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to delete dashboard';
      showError(message);
    }
  };

  const formatRelativeTime = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSeconds = Math.floor(diffMs / 1000);

    // Handle negative or very small values (clock skew)
    if (diffSeconds < 1) {
      return 'just now';
    }

    const diffMinutes = Math.floor(diffSeconds / 60);
    const diffHours = Math.floor(diffMinutes / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffSeconds < 60) {
      return `${diffSeconds}s ago`;
    } else if (diffMinutes < 60) {
      return `${diffMinutes}m ago`;
    } else if (diffHours < 24) {
      return `${diffHours}h ago`;
    } else if (diffDays < 7) {
      return `${diffDays}d ago`;
    } else {
      return date.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      });
    }
  };

  return (
    <Box sx={{ height: '100%', position: 'relative', display: 'flex', flexDirection: 'column' }}>
      {/* Background layer with shimmer */}
      <Box
        className={`grid-shimmer ${!loading ? 'shimmer-stopped' : ''}`}
        sx={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}
      />

      {/* Dashboard Content - scrollable */}
      <Box sx={{ flex: 1, overflow: 'auto', p: { xs: 2, sm: 4, md: 6 }, pb: 10, position: 'relative' }}>
        {!loading && dashboards.length === 0 && (
          <EmptyStateCard
            Icon={DashboardIcon}
            heading="No dashboards yet"
            subtext="Create your first dashboard to start visualizing your analytics data."
            ctaLabel="Create Dashboard"
            onCta={() => navigate('/app/chat?chat_type=create_dashboard')}
          />
        )}

        {!loading && dashboards.length > 0 && (
          <Grid container spacing={3}>
            {dashboards.map((dashboard) => (
              <Grid item xs={12} sm={6} md={4} key={dashboard.id}>
                {dashboard.response && dashboard.query ? (
                  <InlineMetric
                    metricData={{
                      label: dashboard.description || 'Untitled Dashboard',
                      query: dashboard.query,
                      response: { response: dashboard.response },
                      chartConfig: dashboard.chart_config ? { charts: JSON.parse(dashboard.chart_config) } : undefined,
                    }}
                    additionalActions={[
                      {
                        id: 'refresh',
                        label: refreshingId === dashboard.id ? 'Refreshing...' : 'Refresh',
                        icon: RefreshIcon,
                        onClick: () => handleRefreshStandalone(dashboard),
                        disabled: refreshingId === dashboard.id,
                      },
                      {
                        id: 'delete',
                        label: 'Delete',
                        icon: CloseIcon,
                        onClick: () => handleDeleteStandalone(dashboard),
                      },
                    ]}
                    footer={
                      dashboard.last_ran_at ? (
                        <Typography variant="caption" color="text.secondary">
                          Last ran {formatRelativeTime(dashboard.last_ran_at)}
                        </Typography>
                      ) : undefined
                    }
                  />
                ) : (
                  <Paper
                    elevation={0}
                    sx={{
                      borderRadius: 2,
                      border: '1px solid',
                      borderColor: 'divider',
                      overflow: 'hidden',
                    }}
                  >
                    <Box
                      sx={{
                        px: 2,
                        py: 1.5,
                        borderBottom: '1px solid',
                        borderColor: 'divider',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        bgcolor: 'action.hover',
                      }}
                    >
                      <Typography variant="subtitle2" fontWeight={600}>
                        {dashboard.description || 'Untitled Dashboard'}
                      </Typography>
                      </Box>
                    <Box sx={{ p: 3, textAlign: 'center' }}>
                      <Typography variant="body2" color="text.secondary">
                        {!dashboard.query ? 'No query configured.' : 'No data yet.'}
                      </Typography>
                    </Box>
                  </Paper>
                )}
              </Grid>
            ))}
          </Grid>
        )}
      </Box>

      {/* Floating Action Button */}
      {!createDialogOpen && (
        <Fab
          color="primary"
          sx={{
            position: 'fixed',
            bottom: 24,
            right: 24,
          }}
          onClick={() => navigate('/app/chat?chat_type=create_dashboard')}
        >
          <AddIcon size={20} />
        </Fab>
      )}

      {/* Create Dashboard Overlay Sheet - sibling to dashboard content */}
      <Backdrop
        open={createDialogOpen}
        onClick={handleCloseCreateDialog}
        sx={{ position: 'absolute', zIndex: 10, backgroundColor: 'rgba(0, 0, 0, 0.2)' }}
      />
      <Box
        sx={{
          position: 'absolute',
          inset: 0,
          zIndex: 11,
          bgcolor: 'var(--cream, #F8F6F0)',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          opacity: createDialogOpen ? 1 : 0,
          transform: createDialogOpen ? 'translateX(0)' : 'translateX(24px)',
          visibility: createDialogOpen ? 'visible' : 'hidden',
          pointerEvents: createDialogOpen ? 'auto' : 'none',
          transition: createDialogOpen
            ? 'opacity 200ms ease, transform 200ms ease, visibility 0ms'
            : 'opacity 200ms ease, transform 200ms ease, visibility 0ms 200ms',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            p: 2,
            borderBottom: '1px solid',
            borderColor: 'divider',
            flexShrink: 0,
          }}
        >
          <PageHeader title='Create Dashboard' />
          <IconButton onClick={handleCloseCreateDialog} disabled={creating}>
            <CloseIcon size={20} />
          </IconButton>
        </Box>

        <Box sx={{ flex: 1, overflow: 'hidden' }}>
          {createDialogOpen && <DashboardCreationChat onDashboardCreated={() => {
            fetchDashboards();
            setCreateDialogOpen(false);
            showSuccess('Dashboard created successfully.');
          }}/>}
        </Box>
      </Box>
    </Box>
  );
}
