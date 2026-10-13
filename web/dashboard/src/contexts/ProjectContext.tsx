import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { projectService, type Project } from '../services/projectService';
import { useOrganization } from './OrganizationContext';
import { useToast } from './ToastContext';
import { PROJECT_STORAGE_KEY } from '../config/storage';
import { useAutoRefresh } from '../discover/autoRefresh';

/** How often a project that is not ACTIVE yet is checked again. */
const STATUS_POLL_INTERVAL = '10s';

interface ProjectContextType {
  projects: Project[];
  setProjects: (projects: Project[]) => void;
  currentProject: Project | null;
  setCurrentProject: (project: Project | null) => void;
  loading: boolean;
  refreshProjects: () => Promise<void>;
  createProject: (name: string) => Promise<Project>;
}

const ProjectContext = createContext<ProjectContextType | null>(null);

export function ProjectProvider({ children }: { children: ReactNode }) {
  const { currentOrganization } = useOrganization();
  const { showSuccess } = useToast();
  const [projects, setProjects] = useState<Project[]>([]);
  const [currentProject, setCurrentProject] = useState<Project | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (currentOrganization) {
      loadProjects(currentOrganization.id);
    } else {
      setProjects([]);
      setCurrentProject(null);
    }
  }, [currentOrganization?.id]);

  const loadProjects = async (organizationId: string) => {
    setLoading(true);
    try {
      const list = await projectService.list(organizationId);
      setProjects(list);

      const storedId = localStorage.getItem(PROJECT_STORAGE_KEY);
      if (storedId) {
        const stored = list.find(p => p.id === storedId);
        if (stored) {
          setCurrentProject(stored);
          return;
        }
      }

      if (list.length > 0) {
        setCurrentProject(list[0]);
        localStorage.setItem(PROJECT_STORAGE_KEY, list[0].id);
      } else {
        setCurrentProject(null);
      }
    } catch (err) {
      console.error('Failed to load projects:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleSetCurrentProject = (project: Project | null) => {
    setCurrentProject(project);
    if (project) {
      localStorage.setItem(PROJECT_STORAGE_KEY, project.id);
    } else {
      localStorage.removeItem(PROJECT_STORAGE_KEY);
    }
  };

  const refreshProjects = async () => {
    if (currentOrganization) {
      await loadProjects(currentOrganization.id);
    }
  };

  // An operator provisions new projects; watch the current one until it can be queried.
  const pollStatus = async () => {
    if (!currentOrganization || !currentProject) return;
    try {
      const latest = await projectService.get(currentOrganization.id, currentProject.id);
      if (latest.status === currentProject.status) return;
      setCurrentProject((current) => (current?.id === latest.id ? { ...current, ...latest } : current));
      setProjects((list) => list.map((p) => (p.id === latest.id ? { ...p, ...latest } : p)));
      if (latest.status === 'ACTIVE') {
        showSuccess(`${latest.name} is ready`);
      }
    } catch (err) {
      console.error('Failed to check project status:', err);
    }
  };
  useAutoRefresh(
    currentProject && currentProject.status !== 'ACTIVE' ? STATUS_POLL_INTERVAL : null,
    pollStatus,
  );

  const createProject = async (name: string): Promise<Project> => {
    if (!currentOrganization) throw new Error('No organization selected');
    const project = await projectService.create(currentOrganization.id, { name });
    await refreshProjects();
    handleSetCurrentProject(project);
    return project;
  };

  return (
    <ProjectContext.Provider
      value={{
        projects,
        setProjects,
        currentProject,
        setCurrentProject: handleSetCurrentProject,
        loading,
        refreshProjects,
        createProject,
      }}
    >
      {children}
    </ProjectContext.Provider>
  );
}

export function useProject() {
  const context = useContext(ProjectContext);
  if (!context) {
    throw new Error('useProject must be used within a ProjectProvider');
  }
  return context;
}
