import { apiClient } from './apiClient';

/**
 * A project's collector API key, as listed. The secret itself is never
 * returned after creation — only this metadata.
 */
export interface CollectorKey {
  kid: string;
  key_name: string;
  created_at: string;
  /** Id of the user who created the key, if known. */
  created_by: string | null;
  expires_at: string | null;
  revoked_at: string | null;
}

/**
 * A newly created key. `key` is the full secret (`sk_<kid>_<secret>`) and is
 * returned only by the create call — it is not stored anywhere in plaintext.
 */
export interface CreatedCollectorKey extends CollectorKey {
  key: string;
}

interface CreateCollectorKeyRequest {
  key_name: string;
  /** RFC 3339 timestamp in the future; omit for a key that never expires. */
  expires_at?: string;
}

function projectHeaders(organizationId: string, projectId: string): Record<string, string> {
  return { 'X-Organization-Id': organizationId, 'X-Project-Id': projectId };
}

export const collectorKeyService = {
  /** Keys of the project, newest first. Any project member may list. */
  async list(organizationId: string, projectId: string): Promise<CollectorKey[]> {
    return apiClient.get('/project/collector-keys', {}, {
      headers: projectHeaders(organizationId, projectId),
    });
  },

  /** Project admin or org admin only (403 otherwise). */
  async create(organizationId: string, projectId: string, data: CreateCollectorKeyRequest): Promise<CreatedCollectorKey> {
    return apiClient.post('/project/collector-keys', data, {
      headers: projectHeaders(organizationId, projectId),
    });
  },

  /** Project admin or org admin only (403 otherwise). Idempotent. */
  async revoke(organizationId: string, projectId: string, kid: string): Promise<CollectorKey> {
    return apiClient.post(`/project/collector-keys/${encodeURIComponent(kid)}/revoke`, undefined, {
      headers: projectHeaders(organizationId, projectId),
    });
  },
};
