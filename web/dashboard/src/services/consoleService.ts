import { apiClient } from './apiClient';
import type { UserProjectConsole } from '../types/api';

export const consoleService = {
  async list(): Promise<UserProjectConsole[]> {
    return apiClient.get('/project/consoles');
  },

  async get(consoleId: string): Promise<UserProjectConsole> {
    return apiClient.get(`/project/console?console_id=${consoleId}`);
  },

  async create(name: string, console: string): Promise<UserProjectConsole> {
    return apiClient.post('/project/consoles', { name, console });
  },

  async update(id: string, name: string, console: string): Promise<UserProjectConsole> {
    return apiClient.patch('/project/console', { console_id: id, name, console });
  },

  async delete(id: string): Promise<void> {
    await apiClient.delete('/project/console', { params: { console_id: id } });
  },
};
