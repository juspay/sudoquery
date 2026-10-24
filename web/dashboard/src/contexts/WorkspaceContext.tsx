import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { workspaceService, type Workspace } from '../services/workspaceService';
import { useOrganization } from './OrganizationContext';
import { useToast } from './ToastContext';
import { WORKSPACE_STORAGE_KEY } from '../config/storage';
import { useAutoRefresh } from '../discover/autoRefresh';

/** How often a workspace that is not ACTIVE yet is checked again. */
const STATUS_POLL_INTERVAL = '10s';

interface WorkspaceContextType {
  workspaces: Workspace[];
  setWorkspaces: (workspaces: Workspace[]) => void;
  currentWorkspace: Workspace | null;
  setCurrentWorkspace: (workspace: Workspace | null) => void;
  loading: boolean;
  refreshWorkspaces: () => Promise<void>;
  createWorkspace: (name: string) => Promise<Workspace>;
}

const WorkspaceContext = createContext<WorkspaceContextType | null>(null);

export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const { currentOrganization } = useOrganization();
  const { showSuccess } = useToast();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [currentWorkspace, setCurrentWorkspace] = useState<Workspace | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (currentOrganization) {
      loadWorkspaces(currentOrganization.id);
    } else {
      setWorkspaces([]);
      setCurrentWorkspace(null);
    }
  }, [currentOrganization?.id]);

  const loadWorkspaces = async (organizationId: string) => {
    setLoading(true);
    try {
      const list = await workspaceService.list(organizationId);
      setWorkspaces(list);

      const storedId = localStorage.getItem(WORKSPACE_STORAGE_KEY);
      if (storedId) {
        const stored = list.find(p => p.id === storedId);
        if (stored) {
          setCurrentWorkspace(stored);
          return;
        }
      }

      if (list.length > 0) {
        setCurrentWorkspace(list[0]);
        localStorage.setItem(WORKSPACE_STORAGE_KEY, list[0].id);
      } else {
        setCurrentWorkspace(null);
      }
    } catch (err) {
      console.error('Failed to load workspaces:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleSetCurrentWorkspace = (workspace: Workspace | null) => {
    setCurrentWorkspace(workspace);
    if (workspace) {
      localStorage.setItem(WORKSPACE_STORAGE_KEY, workspace.id);
    } else {
      localStorage.removeItem(WORKSPACE_STORAGE_KEY);
    }
  };

  const refreshWorkspaces = async () => {
    if (currentOrganization) {
      await loadWorkspaces(currentOrganization.id);
    }
  };

  // An operator provisions new workspaces; watch the current one until it can be queried.
  const pollStatus = async () => {
    if (!currentOrganization || !currentWorkspace) return;
    try {
      const latest = await workspaceService.get(currentOrganization.id, currentWorkspace.id);
      if (latest.status === currentWorkspace.status) return;
      setCurrentWorkspace((current) => (current?.id === latest.id ? { ...current, ...latest } : current));
      setWorkspaces((list) => list.map((p) => (p.id === latest.id ? { ...p, ...latest } : p)));
      if (latest.status === 'ACTIVE') {
        showSuccess(`${latest.name} is ready`);
      }
    } catch (err) {
      console.error('Failed to check workspace status:', err);
    }
  };
  useAutoRefresh(
    currentWorkspace && currentWorkspace.status !== 'ACTIVE' ? STATUS_POLL_INTERVAL : null,
    pollStatus,
  );

  const createWorkspace = async (name: string): Promise<Workspace> => {
    if (!currentOrganization) throw new Error('No organization selected');
    const workspace = await workspaceService.create(currentOrganization.id, { name });
    await refreshWorkspaces();
    handleSetCurrentWorkspace(workspace);
    return workspace;
  };

  return (
    <WorkspaceContext.Provider
      value={{
        workspaces,
        setWorkspaces,
        currentWorkspace,
        setCurrentWorkspace: handleSetCurrentWorkspace,
        loading,
        refreshWorkspaces,
        createWorkspace,
      }}
    >
      {children}
    </WorkspaceContext.Provider>
  );
}

export function useWorkspace() {
  const context = useContext(WorkspaceContext);
  if (!context) {
    throw new Error('useWorkspace must be used within a WorkspaceProvider');
  }
  return context;
}
