import { Chip, type ChipProps } from '@mui/material';
import { Hourglass, TriangleAlert, Trash2 } from 'lucide-react';
import { EmptyStateCard } from '../shared/EmptyStateCard';
import type { WorkspaceStatus } from '../../services/workspaceService';

type NotReadyStatus = Exclude<WorkspaceStatus, 'ACTIVE'>;

const STATUS_CHIPS: Record<NotReadyStatus, { label: string; color: ChipProps['color'] }> = {
  PENDING: { label: 'Pending', color: 'warning' },
  CREATING: { label: 'Setting up', color: 'warning' },
  ERRORED: { label: 'Setup failed', color: 'error' },
  DELETING: { label: 'Deleting', color: 'default' },
};

/** A small badge for a workspace that cannot be queried yet; nothing once it is ACTIVE. */
export function WorkspaceStatusChip({ status }: { status: WorkspaceStatus }) {
  if (status === 'ACTIVE') return null;
  const { label, color } = STATUS_CHIPS[status];
  return <Chip label={label} size="small" color={color} sx={{ flexShrink: 0 }} />;
}

/** Shown in place of a page while the current workspace cannot be queried. */
export function WorkspaceSetupState({ status }: { status: NotReadyStatus }) {
  switch (status) {
    case 'PENDING':
      return (
        <EmptyStateCard
          Icon={Hourglass}
          heading="Setting up your workspace"
          subtext="An administrator provisions new workspaces. This page updates on its own once the workspace is ready."
        />
      );
    case 'CREATING':
      return (
        <EmptyStateCard
          Icon={Hourglass}
          heading="Setting up your workspace"
          subtext="Its event pipeline and storage are being created. This page updates on its own once the workspace is ready."
        />
      );
    case 'ERRORED':
      return (
        <EmptyStateCard
          Icon={TriangleAlert}
          heading="Workspace setup failed"
          subtext="Ask your administrator to retry setting up this workspace. This page updates on its own once the workspace is ready."
        />
      );
    case 'DELETING':
      return (
        <EmptyStateCard
          Icon={Trash2}
          heading="This workspace is being deleted"
          subtext="Switch to another workspace, or retry the deletion from the workspace settings."
        />
      );
  }
}
