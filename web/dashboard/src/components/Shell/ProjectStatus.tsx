import { Chip, type ChipProps } from '@mui/material';
import { Hourglass, TriangleAlert, Trash2 } from 'lucide-react';
import { EmptyStateCard } from '../shared/EmptyStateCard';
import type { ProjectStatus } from '../../services/projectService';

type NotReadyStatus = Exclude<ProjectStatus, 'ACTIVE'>;

const STATUS_CHIPS: Record<NotReadyStatus, { label: string; color: ChipProps['color'] }> = {
  PENDING: { label: 'Pending', color: 'warning' },
  CREATING: { label: 'Setting up', color: 'warning' },
  ERRORED: { label: 'Setup failed', color: 'error' },
  DELETING: { label: 'Deleting', color: 'default' },
};

/** A small badge for a project that cannot be queried yet; nothing once it is ACTIVE. */
export function ProjectStatusChip({ status }: { status: ProjectStatus }) {
  if (status === 'ACTIVE') return null;
  const { label, color } = STATUS_CHIPS[status];
  return <Chip label={label} size="small" color={color} sx={{ flexShrink: 0 }} />;
}

/** Shown in place of a page while the current project cannot be queried. */
export function ProjectSetupState({ status }: { status: NotReadyStatus }) {
  switch (status) {
    case 'PENDING':
      return (
        <EmptyStateCard
          Icon={Hourglass}
          heading="Setting up your project"
          subtext="An administrator provisions new projects. This page updates on its own once the project is ready."
        />
      );
    case 'CREATING':
      return (
        <EmptyStateCard
          Icon={Hourglass}
          heading="Setting up your project"
          subtext="Its event pipeline and storage are being created. This page updates on its own once the project is ready."
        />
      );
    case 'ERRORED':
      return (
        <EmptyStateCard
          Icon={TriangleAlert}
          heading="Project setup failed"
          subtext="Ask your administrator to retry setting up this project. This page updates on its own once the project is ready."
        />
      );
    case 'DELETING':
      return (
        <EmptyStateCard
          Icon={Trash2}
          heading="This project is being deleted"
          subtext="Switch to another project, or retry the deletion from the project settings."
        />
      );
  }
}
