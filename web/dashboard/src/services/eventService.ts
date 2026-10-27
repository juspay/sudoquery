import { apiClient } from './apiClient';
import { executeQuery } from './clickhouseService';

export interface EventSchemaProperty {
  event_name: string;
  property: string;
  type: string;
}

export interface EventDescription {
  id: string;
  workspace_id: string;
  event_name: string;
  description: string;
  created_at: string;
  updated_at: string;
}

export interface PropertyDescription {
  id: string;
  workspace_id: string;
  event_name: string;
  property_name: string;
  property_type: string;
  description: string;
  created_at: string;
  updated_at: string;
}

function orgHeader(organizationId: string): Record<string, string> {
  return { 'X-Organization-Id': organizationId };
}

export const eventService = {
  async getEventNames(organizationId: string, workspaceId: string): Promise<string[]> {
    return apiClient.get('/events', {}, {
      headers: { ...orgHeader(organizationId), 'X-Workspace-Id': workspaceId },
    });
  },

  async getEventNamesFromClickHouse(): Promise<string[]> {
    const query = 'SELECT DISTINCT event_name FROM events_v1 ORDER BY event_name';
    const result = await executeQuery(query);

    if (!result.success || !result.response?.data) {
      return [];
    }

    return result.response.data.map((row: { event_name: string }) => String(row.event_name ?? ''));
  },

  async getEventSchemaCatalog(): Promise<EventSchemaProperty[]> {
    const query = 'SELECT event_name, property, type FROM event_schema_catalog ORDER BY event_name, property';
    const result = await executeQuery(query);

    if (!result.success || !result.response?.data) {
      return [];
    }

    const seen = new Set<string>();
    const deduped: EventSchemaProperty[] = [];

    for (const row of result.response.data) {
      const key = `${row.event_name}|${row.property}`;
      if (!seen.has(key)) {
        seen.add(key);
        deduped.push({
          event_name: String(row.event_name ?? ''),
          property: String(row.property ?? ''),
          type: String(row.type ?? ''),
        });
      }
    }

    return deduped;
  },

  async getEventDescriptions(workspaceId: string): Promise<EventDescription[]> {
    const response = await apiClient.get<{ event_descriptions: EventDescription[] }>(
      '/event-descriptions',
      {},
      { headers: { 'X-Workspace-Id': workspaceId } }
    );
    return response.event_descriptions || [];
  },

  async upsertEventDescription(workspaceId: string, eventName: string, description: string): Promise<EventDescription> {
    return apiClient.post('/workspace/event-description', { event_name: eventName, description }, {
      headers: { 'X-Workspace-Id': workspaceId },
    });
  },

  async getPropertyDescriptions(workspaceId: string, eventName?: string): Promise<PropertyDescription[]> {
    const params = eventName ? { event_name: eventName } : {};
    const response = await apiClient.get<{ property_descriptions: PropertyDescription[] }>(
      '/property-descriptions',
      params,
      { headers: { 'X-Workspace-Id': workspaceId } }
    );
    return response.property_descriptions || [];
  },

  async upsertPropertyDescription(
    workspaceId: string,
    eventName: string,
    propertyName: string,
    propertyType: string,
    description: string
  ): Promise<PropertyDescription> {
    return apiClient.post('/workspace/property-description', {
      event_name: eventName,
      property_name: propertyName,
      property_type: propertyType,
      description,
    }, {
      headers: { 'X-Workspace-Id': workspaceId },
    });
  },
};
