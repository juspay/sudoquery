import { apiClient } from './apiClient';
import type { Predicate } from '../cdp/predicate';

/**
 * The project's customer profiles and segments, from the CDP through
 * dashboard-server's `/cdp` routes (which scope every call to the current
 * project). Metric values arrive as exact decimal strings; timestamps that
 * are metrics (`last_order_at`, …) are unix epoch seconds.
 */

export type ProfileType = 'customer' | 'visitor';

export interface WorkspaceTotals {
  workspace_members: number | null;
  workspace_customers: number | null;
  workspace_visitors: number | null;
}

export interface CustomerRow {
  profile_id: string;
  profile_type: ProfileType;
  email: string | null;
  phone: string | null;
  total_spend: string | null;
  order_count: string | null;
  last_order_at: string | null;
  created_at?: string;
  entered_at?: string;
}

export interface CustomerPage extends WorkspaceTotals {
  profiles: CustomerRow[];
  next_cursor: string | null;
}

export interface ProfileIdentifier {
  type: string;
  tier: string;
  value: string | null;
  masked_value: string | null;
  status: string;
}

export interface ProfileMetric {
  metric: string;
  display_name: string | null;
  value: string;
}

export interface ProfileSegment {
  segment_id: string;
  display_name: string | null;
  state: 'entered' | 'exited';
  origin: string;
  entered_at: string | null;
  exited_at: string | null;
}

export interface Profile {
  profile_id: string;
  profile_type: ProfileType;
  status: string;
  created_at: string;
  identifiers: ProfileIdentifier[];
  metrics: ProfileMetric[];
  segments: ProfileSegment[];
}

export type SegmentStatus = 'draft' | 'active' | 'paused' | 'archived';

export interface Segment {
  workspace_id: string;
  segment_id: string;
  display_name: string;
  predicate: Predicate;
  is_temporal: boolean;
  status: SegmentStatus;
  created_at: string;
  member_count?: number;
  segment_customers?: number;
  segment_visitors?: number;
}

export interface SegmentMembers extends WorkspaceTotals {
  segment_id: string;
  members: CustomerRow[];
  next_cursor: string | null;
  total_members: number | null;
  segment_customers: number | null;
  segment_visitors: number | null;
}

export interface SegmentPreview {
  matching_customers: number;
  matching_breakdown?: { customers: number; visitors: number };
  workspace_members: number;
  truncated: boolean;
  is_temporal: boolean;
}

export interface MetricDefinition {
  workspace_id: string;
  metric_name: string;
  event_name: string;
  value_kind: string;
  display_name: string | null;
}

export interface EventType {
  event_name: string;
  dedup_entity: string | null;
  dedup_key_paths: string[] | null;
  status: string;
}

export type ValueKind = 'count' | 'property_number' | 'occurred_at_epoch' | 'property_string';

export interface NewMetric {
  metric_name: string;
  event_name: string;
  value_kind: ValueKind;
  value_path?: string;
  window_days?: number;
  aggregation?: 'latest' | 'recent';
  display_name?: string;
}

export interface MetricDefinitionDetail extends MetricDefinition {
  value_path: string | null;
  window_days: number | null;
  aggregation: string | null;
}

export interface NewSegment {
  display_name: string;
  predicate: Predicate;
  status?: SegmentStatus;
}

function pageParams(cursor?: string | null, profileType?: ProfileType | null): Record<string, string> {
  const params: Record<string, string> = {};
  if (cursor) params.cursor = cursor;
  if (profileType) params.profile_type = profileType;
  return params;
}

export const cdpService = {
  /**
   * One page of profiles. `/cdp/summary` ignores `profile_type`, so a filtered
   * list comes from `/cdp/profiles` (which has no workspace totals; see `totals`).
   */
  customers(cursor?: string | null, profileType?: ProfileType | null): Promise<CustomerPage> {
    if (profileType) {
      return apiClient
        .get<{ profiles: CustomerRow[]; next_cursor: string | null }>('/cdp/profiles', {
          limit: '50',
          ...pageParams(cursor, profileType),
        })
        .then((page) => ({ ...page, workspace_members: null, workspace_customers: null, workspace_visitors: null }));
    }
    return apiClient.get('/cdp/summary', { limit: '50', ...pageParams(cursor) });
  },

  totals(): Promise<WorkspaceTotals> {
    return apiClient.get('/cdp/summary', { limit: '1' });
  },

  lookup(type: string, value: string): Promise<Profile> {
    return apiClient.get('/cdp/profiles/lookup', { type, value });
  },

  profile(profileId: string): Promise<Profile> {
    return apiClient.get(`/cdp/profiles/${encodeURIComponent(profileId)}`);
  },

  async segments(): Promise<Segment[]> {
    const page = await apiClient.get<{ segments: Segment[] }>('/cdp/segments', { limit: '500' });
    return page.segments;
  },

  segment(segmentId: string): Promise<Segment> {
    return apiClient.get(`/cdp/segments/${encodeURIComponent(segmentId)}`);
  },

  members(segmentId: string, cursor?: string | null): Promise<SegmentMembers> {
    return apiClient.get(`/cdp/segments/${encodeURIComponent(segmentId)}/members`, {
      limit: '50',
      ...pageParams(cursor),
    });
  },

  preview(predicate: Predicate, signal?: AbortSignal): Promise<SegmentPreview> {
    return apiClient.post('/cdp/segments/preview', { predicate }, { signal });
  },

  createSegment(segmentId: string, segment: NewSegment): Promise<{ segment_id: string }> {
    return apiClient.put(`/cdp/segments/${encodeURIComponent(segmentId)}`, segment);
  },

  setSegmentStatus(segmentId: string, status: SegmentStatus): Promise<{ status: SegmentStatus }> {
    return apiClient.put(`/cdp/segments/${encodeURIComponent(segmentId)}/status`, { status });
  },

  async metricDefinitions(): Promise<MetricDefinitionDetail[]> {
    const body = await apiClient.get<{ metric_definitions: MetricDefinitionDetail[] }>('/cdp/metric-definitions');
    return body.metric_definitions;
  },

  // The organization's vocabulary — writes need organization admin.

  async eventTypes(): Promise<EventType[]> {
    const body = await apiClient.get<{ event_types: EventType[] }>('/cdp/event-types');
    return body.event_types;
  },

  putEventType(eventName: string, dedup?: { entity: string; keyPath: string }): Promise<unknown> {
    return apiClient.put(
      `/cdp/event-types/${encodeURIComponent(eventName)}`,
      dedup ? { dedup_entity: dedup.entity, dedup_key_paths: [dedup.keyPath] } : {},
    );
  },

  deleteEventType(eventName: string): Promise<unknown> {
    return apiClient.delete(`/cdp/event-types/${encodeURIComponent(eventName)}`);
  },

  putMetric(metric: NewMetric): Promise<unknown> {
    return apiClient.put('/cdp/metric-definitions', metric);
  },

  deleteMetric(metricName: string, eventName: string): Promise<unknown> {
    return apiClient.delete('/cdp/metric-definitions', {
      params: { metric_name: metricName, event_name: eventName },
    });
  },
};
