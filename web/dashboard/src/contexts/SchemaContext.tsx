import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { useProject } from './ProjectContext';
import { eventService, type EventDescription, type PropertyDescription, type EventSchemaProperty } from '../services/eventService';
import { executeQuery } from '../services/clickhouseService';

interface SchemaStatus {
  eventsNeedingDescription: number;
  propertiesNeedingDescription: number;
  loading: boolean;
  refresh: () => Promise<void>;
}

const SchemaContext = createContext<SchemaStatus | null>(null);

export function SchemaProvider({ children }: { children: ReactNode }) {
  const { currentProject } = useProject();
  const [eventsNeedingDescription, setEventsNeedingDescription] = useState(0);
  const [propertiesNeedingDescription, setPropertiesNeedingDescription] = useState(0);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    if (!currentProject?.id) {
      setEventsNeedingDescription(0);
      setPropertiesNeedingDescription(0);
      return;
    }

    setLoading(true);
    try {
      // Fetch all data in parallel
      const [eventNamesResult, schemaCatalogResult, eventDescs, propDescs] = await Promise.all([
        executeQuery('SELECT DISTINCT event_name FROM events_v1'),
        executeQuery('SELECT event_name, property FROM event_schema_catalog'),
        eventService.getEventDescriptions(currentProject.id),
        eventService.getPropertyDescriptions(currentProject.id),
      ]);

      // Extract event names
      const eventNames = eventNamesResult.success && eventNamesResult.response?.data
        ? eventNamesResult.response.data.map((row: { event_name: string }) => String(row.event_name ?? ''))
        : [];

      // Extract properties (deduplicated)
      const propertyKeys = new Set<string>();
      if (schemaCatalogResult.success && schemaCatalogResult.response?.data) {
        for (const row of schemaCatalogResult.response.data) {
          const key = `${row.event_name}|${row.property}`;
          propertyKeys.add(key);
        }
      }

      // Create lookup sets
      const eventDescSet = new Set(eventDescs.map((d) => d.event_name));
      const propDescSet = new Set(propDescs.map((d) => `${d.event_name}|${d.property_name}`));

      // Calculate missing counts
      const eventsMissing = eventNames.filter((name) => !eventDescSet.has(name)).length;
      const propsMissing = Array.from(propertyKeys).filter((key) => !propDescSet.has(key)).length;

      setEventsNeedingDescription(eventsMissing);
      setPropertiesNeedingDescription(propsMissing);
    } catch (err) {
      console.error('Failed to fetch schema status:', err);
    } finally {
      setLoading(false);
    }
  }, [currentProject?.id]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <SchemaContext.Provider
      value={{
        eventsNeedingDescription,
        propertiesNeedingDescription,
        loading,
        refresh,
      }}
    >
      {children}
    </SchemaContext.Provider>
  );
}

export function useSchemaStatus() {
  const context = useContext(SchemaContext);
  if (!context) {
    throw new Error('useSchemaStatus must be used within a SchemaProvider');
  }
  return context;
}
