import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { organizationService, type Organization } from '../services/organizationService';
import { useAuth } from './AuthContext';

interface OrganizationContextType {
  organizations: Organization[];
  currentOrganization: Organization | null;
  setCurrentOrganization: (org: Organization | null) => void;
  loading: boolean;
  initialized: boolean;
  error: string | null;
  refreshOrganizations: () => Promise<void>;
  createOrganization: (name: string, slug: string) => Promise<Organization>;
}

const OrganizationContext = createContext<OrganizationContextType | null>(null);

const STORAGE_KEY = 'current_organization_id';

export function OrganizationProvider({ children }: { children: ReactNode }) {
  const { isAuthenticated } = useAuth();
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [currentOrganization, setCurrentOrganization] = useState<Organization | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [initialized, setInitialized] = useState(false);

  // Load organizations when authenticated
  useEffect(() => {
    if (isAuthenticated && !initialized) {
      loadOrganizations();
    } else if (!isAuthenticated) {
      setOrganizations([]);
      setCurrentOrganization(null);
      setError(null);
      setLoading(false);
      setInitialized(false);
    }
  }, [isAuthenticated, initialized]);

  const loadOrganizations = async () => {
    setLoading(true);
    setError(null);
    try {
      const orgs = await organizationService.list();
      console.log('[OrganizationContext] Loaded orgs:', orgs);
      setOrganizations(orgs);

      // Restore previously selected organization
      const storedOrgId = localStorage.getItem(STORAGE_KEY);
      if (storedOrgId) {
        const storedOrg = orgs.find(o => o.id === storedOrgId);
        if (storedOrg) {
          console.log('[OrganizationContext] Restored stored org:', storedOrg.id);
          setCurrentOrganization(storedOrg);
          return;
        }
      }

      // Default to first organization if available
      if (orgs.length > 0) {
        console.log('[OrganizationContext] Setting first org:', orgs[0].id);
        setCurrentOrganization(orgs[0]);
        localStorage.setItem(STORAGE_KEY, orgs[0].id);
      } else {
        console.log('[OrganizationContext] No organizations found');
        setCurrentOrganization(null);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load organizations';
      console.error('Failed to load organizations:', err);
      setError(message);
    } finally {
      setLoading(false);
      setInitialized(true);
    }
  };

  const handleSetCurrentOrganization = (org: Organization | null) => {
    setCurrentOrganization(org);
    if (org) {
      localStorage.setItem(STORAGE_KEY, org.id);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  };

  const refreshOrganizations = async () => {
    await loadOrganizations();
  };

  const createOrganization = async (name: string, slug: string): Promise<Organization> => {
    const org = await organizationService.create({ name, slug });
    await refreshOrganizations();
    handleSetCurrentOrganization(org);
    return org;
  };

  return (
    <OrganizationContext.Provider
      value={{
        organizations,
        currentOrganization,
        setCurrentOrganization: handleSetCurrentOrganization,
        loading,
        initialized,
        error,
        refreshOrganizations,
        createOrganization,
      }}
    >
      {children}
    </OrganizationContext.Provider>
  );
}

export function useOrganization() {
  const context = useContext(OrganizationContext);
  if (!context) {
    throw new Error('useOrganization must be used within an OrganizationProvider');
  }
  return context;
}
