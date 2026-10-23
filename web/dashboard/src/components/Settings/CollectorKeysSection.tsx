import { useEffect, useState } from 'react';
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Link,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { endOfDay, isValid } from 'date-fns';
import { ExternalLink, Plus as AddIcon } from 'lucide-react';
import { Link as RouterLink } from 'react-router-dom';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useProject } from '../../contexts/ProjectContext';
import { useToast } from '../../contexts/ToastContext';
import { ApiError, apiErrorMessage } from '../../services/apiClient';
import { collectorKeyService, type CollectorKey, type CreatedCollectorKey } from '../../services/collectorKeyService';
import { CopyableId } from '../shared/CopyableId';
import {
  colorBlueDark,
  colorBluePale,
  colorCream,
  colorCream2,
  colorCream3,
  colorInk60,
  colorRoseDark,
  colorRosePale,
  fontFamilyMono,
} from '../../theme/tokens';

const MAX_KEY_NAME_LENGTH = 255;
const CREATE_FORM_ID = 'create-collector-key-form';

type KeyStatus = 'active' | 'expired' | 'revoked';

const statusConfig: Record<KeyStatus, { label: string; bg: string; color: string }> = {
  active: { label: 'Active', bg: colorBluePale, color: colorBlueDark },
  expired: { label: 'Expired', bg: colorCream3, color: colorInk60 },
  revoked: { label: 'Revoked', bg: colorRosePale, color: colorRoseDark },
};

function keyStatus(key: CollectorKey, now: number): KeyStatus {
  if (key.revoked_at) return 'revoked';
  if (key.expires_at && new Date(key.expires_at).getTime() <= now) return 'expired';
  return 'active';
}

const dateFormat = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
const dateTimeFormat = new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short' });

/** The key's metadata without its secret, for keeping in the list. */
function withoutSecret(created: CreatedCollectorKey): CollectorKey {
  return {
    kid: created.kid,
    key_name: created.key_name,
    created_at: created.created_at,
    created_by: created.created_by,
    expires_at: created.expires_at,
    revoked_at: created.revoked_at,
  };
}

/** Non-admins get a 403 from create/revoke; say why rather than echo the server. */
function keyErrorMessage(err: unknown, fallback: string): string {
  if (err instanceof ApiError && err.status === 403) {
    return 'Only project admins and organization admins can manage collector API keys.';
  }
  return apiErrorMessage(err, fallback);
}

function DateText({ value }: { value: string }) {
  const date = new Date(value);
  return (
    <Typography variant="body2" color="text.secondary" title={dateTimeFormat.format(date)} sx={{ whiteSpace: 'nowrap' }}>
      {dateFormat.format(date)}
    </Typography>
  );
}

function KeyStatusBadge({ status, revokedAt }: { status: KeyStatus; revokedAt: string | null }) {
  const config = statusConfig[status];
  return (
    <Tooltip title={revokedAt ? `Revoked ${dateTimeFormat.format(new Date(revokedAt))}` : ''}>
      <Chip
        size="small"
        label={config.label}
        sx={{
          bgcolor: config.bg,
          color: config.color,
          fontWeight: 600,
          borderRadius: 1,
          height: 24,
          '& .MuiChip-label': { px: 1 },
        }}
      />
    </Tooltip>
  );
}

/**
 * The current project's collector API keys: what clients send as
 * `Authorization: Bearer <key>` to the events collector's authenticated
 * endpoint. Create and revoke are shown to everyone; the server allows them
 * only for project and org admins. Mount it with `key={projectId}` so that
 * switching projects starts from a clean state.
 */
export function CollectorKeysSection() {
  const { currentOrganization } = useOrganization();
  const { currentProject } = useProject();
  const { showSuccess } = useToast();
  const organizationId = currentOrganization?.id;
  const projectId = currentProject?.id;

  const [keys, setKeys] = useState<CollectorKey[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // When `keys` last changed; key expiry is judged against it.
  const [keysAt, setKeysAt] = useState(0);

  // Create dialog
  const [createOpen, setCreateOpen] = useState(false);
  const [keyName, setKeyName] = useState('');
  const [expiresOn, setExpiresOn] = useState<Date | null>(null);
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // A just-created key, secret included. It is shown once and dropped from
  // state as soon as its dialog has closed.
  const [createdKey, setCreatedKey] = useState<CreatedCollectorKey | null>(null);
  const [createdKeyOpen, setCreatedKeyOpen] = useState(false);

  // Revoke dialog
  const [revokeTarget, setRevokeTarget] = useState<CollectorKey | null>(null);
  const [revokeOpen, setRevokeOpen] = useState(false);
  const [revoking, setRevoking] = useState(false);
  const [revokeError, setRevokeError] = useState<string | null>(null);

  useEffect(() => {
    if (!organizationId || !projectId) return;

    let cancelled = false;
    collectorKeyService
      .list(organizationId, projectId)
      .then((list) => {
        if (cancelled) return;
        setKeys(list);
        setKeysAt(Date.now());
      })
      .catch((err: unknown) => {
        if (!cancelled) setLoadError(keyErrorMessage(err, 'Failed to load collector API keys'));
      });
    return () => {
      cancelled = true;
    };
  }, [organizationId, projectId]);

  const openCreateDialog = () => {
    setKeyName('');
    setExpiresOn(null);
    setCreateError(null);
    setCreateOpen(true);
  };

  const closeCreateDialog = () => {
    if (!creating) setCreateOpen(false);
  };

  const canCreate = keyName.trim().length > 0 && (expiresOn === null || isValid(expiresOn)) && !creating;

  const handleCreate = async () => {
    if (!organizationId || !projectId || !canCreate) return;

    setCreating(true);
    setCreateError(null);
    try {
      const created = await collectorKeyService.create(organizationId, projectId, {
        key_name: keyName.trim(),
        // The key works through the whole of the chosen (local) day.
        ...(expiresOn ? { expires_at: endOfDay(expiresOn).toISOString() } : {}),
      });
      setKeys((prev) => [withoutSecret(created), ...(prev ?? [])]);
      setKeysAt(Date.now());
      setCreateOpen(false);
      setCreatedKey(created);
      setCreatedKeyOpen(true);
    } catch (err) {
      setCreateError(keyErrorMessage(err, 'Failed to create key'));
    } finally {
      setCreating(false);
    }
  };

  const openRevokeDialog = (key: CollectorKey) => {
    setRevokeTarget(key);
    setRevokeError(null);
    setRevokeOpen(true);
  };

  const closeRevokeDialog = () => {
    if (!revoking) setRevokeOpen(false);
  };

  const handleRevoke = async () => {
    if (!organizationId || !projectId || !revokeTarget) return;

    setRevoking(true);
    setRevokeError(null);
    try {
      const revoked = await collectorKeyService.revoke(organizationId, projectId, revokeTarget.kid);
      setKeys((prev) => prev?.map((k) => (k.kid === revoked.kid ? revoked : k)) ?? prev);
      setKeysAt(Date.now());
      setRevokeOpen(false);
      showSuccess(`Key "${revoked.key_name}" revoked.`);
    } catch (err) {
      setRevokeError(keyErrorMessage(err, 'Failed to revoke key'));
    } finally {
      setRevoking(false);
    }
  };

  const loading = keys === null && loadError === null;

  return (
    <Box sx={{ mt: 4 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 2, mb: 2 }}>
        <Typography variant="h6" fontWeight={600}>
          Collector API keys
        </Typography>
        <Button
          variant="contained"
          size="small"
          startIcon={<AddIcon size={20} />}
          onClick={openCreateDialog}
        >
          Create key
        </Button>
      </Box>

      <Typography variant="body2" sx={{ mb: 2, color: 'text.secondary' }}>
        Clients authenticate to the events collector with one of these keys. Revoke a key to stop it from sending
        events.{' '}
        <Link component={RouterLink} to="/docs" sx={{ fontWeight: 500, display: 'inline-flex', alignItems: 'center', gap: 0.5 }}>
          Learn how to integrate the SDK
          <ExternalLink size={12} />
        </Link>
      </Typography>

      {loading && (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
          <CircularProgress size={24} />
        </Box>
      )}

      {loadError && <Alert severity="error">{loadError}</Alert>}

      {keys && (
        <Box sx={{ border: `1px solid ${colorCream2}`, borderRadius: 2, overflow: 'hidden', bgcolor: '#fff' }}>
          <TableContainer>
            <Table sx={{ minWidth: 600 }} aria-label="collector API keys">
              <TableHead sx={{ bgcolor: 'rgba(28,25,23,0.02)' }}>
                <TableRow>
                  <TableCell>Name</TableCell>
                  <TableCell>Key ID</TableCell>
                  <TableCell>Created</TableCell>
                  <TableCell>Expires</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {keys.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} sx={{ border: 0, py: 4, textAlign: 'center' }}>
                      <Typography variant="body2" color="text.secondary">
                        No collector API keys yet. Create one to start sending authenticated events.
                      </Typography>
                    </TableCell>
                  </TableRow>
                ) : (
                  keys.map((key) => {
                    const status = keyStatus(key, keysAt);
                    return (
                      <TableRow key={key.kid} sx={{ '&:last-child td, &:last-child th': { border: 0 } }}>
                        <TableCell component="th" scope="row">
                          <Typography variant="body2" fontWeight={600} sx={{ wordBreak: 'break-word' }}>
                            {key.key_name}
                          </Typography>
                        </TableCell>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: fontFamilyMono, fontSize: '0.8125rem', whiteSpace: 'nowrap' }}>
                            {key.kid}
                          </Typography>
                        </TableCell>
                        <TableCell>
                          <DateText value={key.created_at} />
                        </TableCell>
                        <TableCell>
                          {key.expires_at ? (
                            <DateText value={key.expires_at} />
                          ) : (
                            <Typography variant="body2" color="text.secondary">
                              Never
                            </Typography>
                          )}
                        </TableCell>
                        <TableCell>
                          <KeyStatusBadge status={status} revokedAt={key.revoked_at} />
                        </TableCell>
                        <TableCell align="right">
                          {status === 'active' && (
                            <Button size="small" color="error" onClick={() => openRevokeDialog(key)}>
                              Revoke
                            </Button>
                          )}
                        </TableCell>
                      </TableRow>
                    );
                  })
                )}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      )}

      {/* Create Dialog */}
      <Dialog open={createOpen} onClose={closeCreateDialog} maxWidth="sm" fullWidth>
        <DialogTitle>
          Create Collector API Key
          <Typography variant="caption" display="block" color="text.secondary" sx={{ mt: 0.5 }}>
            For Project: {currentProject?.name}
          </Typography>
        </DialogTitle>
        <DialogContent>
          {createError && <Alert severity="error" sx={{ mb: 2 }}>{createError}</Alert>}
          <Box
            component="form"
            id={CREATE_FORM_ID}
            noValidate
            onSubmit={(e) => {
              e.preventDefault();
              handleCreate();
            }}
            sx={{ display: 'flex', flexDirection: 'column', gap: 3, mt: 1 }}
          >
            <TextField
              label="Name"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="e.g. Production web app"
              helperText="Helps you recognise where the key is used."
              disabled={creating}
              required
              fullWidth
              autoFocus
              slotProps={{ htmlInput: { maxLength: MAX_KEY_NAME_LENGTH } }}
            />
            <LocalizationProvider dateAdapter={AdapterDateFns}>
              <DatePicker
                label="Expires on (optional)"
                value={expiresOn}
                onChange={(value) => setExpiresOn(value)}
                disablePast
                disabled={creating}
                slotProps={{
                  textField: {
                    fullWidth: true,
                    helperText: 'The key stops working at the end of this day. Leave empty for a key that never expires.',
                  },
                  field: { clearable: true },
                }}
              />
            </LocalizationProvider>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={closeCreateDialog} disabled={creating}>
            Cancel
          </Button>
          <Button type="submit" form={CREATE_FORM_ID} variant="contained" disabled={!canCreate}>
            {creating ? 'Creating...' : 'Create Key'}
          </Button>
        </DialogActions>
      </Dialog>

      {/* Created Key Dialog — the only time the secret is shown */}
      <Dialog
        open={createdKeyOpen}
        onClose={(_, reason) => {
          // A stray click outside must not throw away a key that can't be shown again.
          if (reason !== 'backdropClick') setCreatedKeyOpen(false);
        }}
        maxWidth="sm"
        fullWidth
        slotProps={{ transition: { onExited: () => setCreatedKey(null) } }}
      >
        <DialogTitle>
          Key Created
          <Typography variant="caption" display="block" color="text.secondary" sx={{ mt: 0.5 }}>
            {createdKey?.key_name}
          </Typography>
        </DialogTitle>
        <DialogContent>
          <Alert severity="warning" sx={{ mb: 3 }}>
            Copy this key now. It won't be shown again — if you lose it, revoke it and create a new one.
          </Alert>
          {createdKey && (
            <Paper elevation={0} sx={{ bgcolor: colorBluePale, p: 2.5, borderRadius: 2, mb: 3 }}>
              <CopyableId label="API key" value={createdKey.key} />
            </Paper>
          )}
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            Send events to the collector with the key as a Bearer token:
          </Typography>
          <Box
            component="pre"
            sx={{
              m: 0,
              p: 1.5,
              bgcolor: colorCream,
              border: `1px solid ${colorCream3}`,
              borderRadius: 1,
              fontFamily: fontFamilyMono,
              fontSize: '0.8125rem',
              overflowX: 'auto',
            }}
          >
            {'POST /v1/events/authenticated\nAuthorization: Bearer <key>'}
          </Box>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2.5 }}>
          <Button variant="contained" onClick={() => setCreatedKeyOpen(false)}>
            Done
          </Button>
        </DialogActions>
      </Dialog>

      {/* Revoke Confirmation Dialog */}
      <Dialog
        open={revokeOpen}
        onClose={closeRevokeDialog}
        slotProps={{ paper: { sx: { borderRadius: 2, maxWidth: 480 } } }}
      >
        <DialogTitle sx={{ color: 'error.main', fontWeight: 600, pb: 1 }}>
          Revoke Key
        </DialogTitle>
        <DialogContent>
          {revokeError && <Alert severity="error" sx={{ mb: 2 }}>{revokeError}</Alert>}
          <DialogContentText sx={{ color: 'text.secondary', lineHeight: 1.6 }}>
            Are you sure you want to revoke "{revokeTarget?.key_name}" (
            <Box component="span" sx={{ fontFamily: fontFamilyMono }}>{revokeTarget?.kid}</Box>
            )? Clients using this key will no longer be able to send events. This action cannot be undone.
          </DialogContentText>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2.5, gap: 1 }}>
          <Button onClick={closeRevokeDialog} color="inherit" variant="outlined" disabled={revoking}>
            Cancel
          </Button>
          <Button onClick={handleRevoke} color="error" variant="contained" disabled={revoking}>
            {revoking ? 'Revoking...' : 'Revoke'}
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
