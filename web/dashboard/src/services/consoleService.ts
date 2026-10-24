import { apiClient } from './apiClient';
import type { UserWorkspaceConsole } from '../types/api';

export const consoleService = {
  async list(): Promise<UserWorkspaceConsole[]> {
    return apiClient.get('/workspace/consoles');
  },

  async get(consoleId: string): Promise<UserWorkspaceConsole> {
    return apiClient.get(`/workspace/console?console_id=${consoleId}`);
  },

  async create(name: string, console: string): Promise<UserWorkspaceConsole> {
    return apiClient.post('/workspace/consoles', { name, console });
  },

  async update(id: string, name: string, console: string): Promise<UserWorkspaceConsole> {
    return apiClient.patch('/workspace/console', { console_id: id, name, console });
  },

  async delete(id: string): Promise<void> {
    await apiClient.delete('/workspace/console', { params: { console_id: id } });
  },
};
