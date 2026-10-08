import type { QueryResponse } from '../types/api';
import { trackQueryExecuted, trackQueryError } from '../utils/analytics';
import { PROJECT_STORAGE_KEY } from '../config/storage';
import { apiClient } from './apiClient';

const QUERY_ENDPOINT = '/query';

export async function executeQuery(query: string): Promise<QueryResponse> {
  const projectId = localStorage.getItem(PROJECT_STORAGE_KEY);
  const headers: Record<string, string> = {};
  if (projectId) {
    headers['X-Project-Id'] = projectId;
  }

  const startTime = performance.now();

  try {
    const data: QueryResponse = await apiClient.post(QUERY_ENDPOINT, { query }, { headers });

    // Calculate query latency
    const latencyMs = Math.round(performance.now() - startTime);

    // Track successful query execution
    const resultRows = data.response?.data?.length || 0;
    trackQueryExecuted(query.length, resultRows, latencyMs);

    return data;
  } catch (error) {
    const latencyMs = Math.round(performance.now() - startTime);
    const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred while executing query';
    trackQueryError(errorMessage, latencyMs);

    return {
      success: false,
      error: errorMessage,
    };
  }
}