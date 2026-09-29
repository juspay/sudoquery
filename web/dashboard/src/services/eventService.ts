import { apiClient } from './apiClient';
import { executeQuery } from './clickhouseService';

export interface EventSchemaProperty {
  event_name: string;
  property: string;
  type: string;
}

export interface EventDescription {
  id: string;
  project_id: string;
  event_name: string;
  description: string;
  created_at: string;
  updated_at: string;
}

export interface PropertyDescription {
  id: string;
  project_id: string;
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
  async getEventNames(organizationId: string, projectId: string): Promise<string[]> {
    return apiClient.get('/events', {}, {
      headers: { ...orgHeader(organizationId), 'X-Project-Id': projectId },
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

  async getEventDescriptions(projectId: string): Promise<EventDescription[]> {
    const response = await apiClient.get<{ event_descriptions: EventDescription[] }>(
      '/event-descriptions',
      {},
      { headers: { 'X-Project-Id': projectId } }
    );
    return response.event_descriptions || [];
  },

  async upsertEventDescription(projectId: string, eventName: string, description: string): Promise<EventDescription> {
    return apiClient.post('/project/event-description', { event_name: eventName, description }, {
      headers: { 'X-Project-Id': projectId },
    });
  },

  async getPropertyDescriptions(projectId: string, eventName?: string): Promise<PropertyDescription[]> {
    const params = eventName ? { event_name: eventName } : {};
    const response = await apiClient.get<{ property_descriptions: PropertyDescription[] }>(
      '/property-descriptions',
      params,
      { headers: { 'X-Project-Id': projectId } }
    );
    return response.property_descriptions || [];
  },

  async upsertPropertyDescription(
    projectId: string,
    eventName: string,
    propertyName: string,
    propertyType: string,
    description: string
  ): Promise<PropertyDescription> {
    return apiClient.post('/project/property-description', {
      event_name: eventName,
      property_name: propertyName,
      property_type: propertyType,
      description,
    }, {
      headers: { 'X-Project-Id': projectId },
    });
  },
};
