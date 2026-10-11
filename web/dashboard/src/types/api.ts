import type { DataRow } from './chart';

interface QueryMeta {
  name: string;
  type: string;
}

export interface QueryStatistics {
  bytes_read: number;
  elapsed: number;
  rows_read: number;
}

export interface QueryResponseData {
  data: DataRow[];
  meta: QueryMeta[];
  rows: number;
  statistics: QueryStatistics;
}

export interface QueryResponse {
  response?: QueryResponseData;
  success: boolean;
  error?: string;
  status?: number;
}

export interface UserProjectConsole {
  id: string;
  name: string | null;
  created_at: string;
  updated_at: string;
  user_id?: string;
  project_id?: string;
  console?: string | null;
}