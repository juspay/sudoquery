import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { projectService, type Project } from '../services/projectService';
import { useOrganization } from './OrganizationContext';
import { PROJECT_STORAGE_KEY } from '../config/storage';

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
